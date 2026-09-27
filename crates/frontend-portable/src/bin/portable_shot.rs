#![forbid(unsafe_code)]

//! Headless screenshot tool for the portable frontend, canvas backend (#459).
//!
//! `emusic-shot` (`crates/app`, #118) only covers the native Win32 frontend:
//! it drives `win32ui`'s `Windows.Graphics.Capture` path, which does not exist
//! off Windows. This tool instead runs the real
//! [`Win32App`](emusic_frontend_portable::app::Win32App) — the toolkit-agnostic
//! shared app type, despite the name — against the deterministic mock
//! backends on `xui`'s software `canvas` backend
//! (`xui::xui_canvas::WinitBackend`), and captures with
//! [`Ui::capture`](xui::xui_core::app::Ui::capture). That call re-composites
//! the window into an offscreen `tiny-skia` surface in software: no OS screen
//! grab, window focus or visibility is needed, so it works the same on a
//! headless CI runner (the `macos` job, folding in the screenshot from #444)
//! as on a dev box.
//!
//! Only the canvas backend is covered: the win32 xui backend has
//! `emusic-shot`'s native path as a close cousin already and is lower
//! priority (#459). The Properties and tag-editor dialog capture modes
//! (`--properties`/`--tag-editor` on `emusic-shot`) are also out of scope for
//! this pass — a follow-up, not implemented here.
//!
//! One window per process: `--all` re-invokes this binary once per view, same
//! as `emusic-shot`. `xui`'s `winit`-backed event loop cannot be restarted
//! within a process once it exits, so each view needs a fresh process anyway.
//!
//! Kept as a separate binary, gated behind the `shot` feature, so the PNG
//! encoder and `clap` never end up in the real `emusic` dependency tree.
//!
//! ```text
//! cargo run -p emusic-frontend-portable --features shot --bin emusic-portable-shot -- \
//!     --view music --theme dark --out target/shots/music.png
//! cargo run -p emusic-frontend-portable --features shot --bin emusic-portable-shot -- --all
//! ```

use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, anyhow, bail};
use clap::Parser;

use emusic_ui::backend;
use emusic_ui::config::Config;
use emusic_ui::state::{
    Accent, Appearance, Density, FontSize, SettingsTab, Theme, View, VisualizerMode,
};
use emusic_ui::waker::WakerSlot;

use emusic_frontend_portable::app::{Msg, Win32App};
use emusic_frontend_portable::views::settings::SettingsMsg;
use emusic_frontend_portable::window::window_spec;
use xui::xui_core::app::{App, Ui, run_app};
use xui::xui_core::backend::{Backend, TimerId};
use xui::xui_core::image::Image;

/// How long the window is left to settle (create its widgets, lay out) before
/// the capture. The canvas backend paints synchronously as widgets are built,
/// so this is only a small head start.
const SETTLE: Duration = Duration::from_millis(300);
/// The tool's own tick interval, so it keeps driving even when the shell's
/// repaint timer is idle (a stopped player schedules no frame).
const TICK_MS: u32 = 40;

#[derive(Parser, Debug)]
#[command(name = "emusic-portable-shot")]
struct Cli {
    /// View to render, e.g. `music`, `albums`, `now-playing`. Ignored with
    /// `--all`.
    #[arg(long)]
    view: Option<String>,

    /// Render every view into `--out`'s directory, one PNG per view. Runs one
    /// fresh process per view.
    #[arg(long)]
    all: bool,

    #[arg(long, value_enum, default_value = "dark")]
    theme: ThemeArg,

    /// Accent colour: a preset name or `#rrggbb`.
    #[arg(long, value_parser = parse_accent, default_value = "orange")]
    accent: Accent,

    /// UI font-size scale (#309): `small`, `default`, `large`, `larger`.
    #[arg(long, value_parser = parse_font_size, default_value = "default")]
    font_size: FontSize,

    /// List density (#309): `compact`, `comfortable`, `spacious`.
    #[arg(long, value_parser = parse_density, default_value = "comfortable")]
    density: Density,

    /// Turn zebra striping off for the capture (#309). Striping is on by
    /// default.
    #[arg(long)]
    no_zebra: bool,

    /// Settings tab to show: `library`, `appearance`, `associations`,
    /// `playback`, `about`. Only meaningful for the Settings view.
    #[arg(long, value_parser = parse_settings_tab)]
    settings_tab: Option<SettingsTab>,

    /// Show the top-bar visualizer in this mode (`spectrum`, `oscilloscope`).
    #[arg(long, value_parser = parse_visualizer)]
    visualizer: Option<VisualizerMode>,

    /// `<width>x<height>`, e.g. `1100x720`.
    #[arg(long, default_value = "1100x720")]
    size: String,

    /// Output PNG path (single view) or directory (`--all`).
    #[arg(long, default_value = "target/shots/shot.png")]
    out: PathBuf,
}

/// The CLI's theme spelling, mapped to the shell's palette.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ThemeArg {
    Dark,
    Light,
}

impl ThemeArg {
    fn shell(self) -> Theme {
        match self {
            Self::Dark => Theme::Dark,
            Self::Light => Theme::Light,
        }
    }

    /// Opaque stand-in for the window background behind any region the
    /// canvas backend leaves transparent (it composites the whole window in
    /// software, so this is mostly a safety net; see [`flatten`]).
    fn backdrop(self) -> [u8; 3] {
        match self {
            Self::Dark => [32, 32, 32],
            Self::Light => [243, 243, 243],
        }
    }

    /// The `--theme` value to pass on to a child process.
    fn slug(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    // Validate `--size` up front so `--all` fails fast rather than in each
    // child.
    parse_size(&cli.size)?;

    if cli.all {
        return run_all(&cli);
    }

    let view = match cli.view.as_deref() {
        Some(slug) => parse_view(slug)?,
        None => View::Music,
    };
    let (width, height) = parse_size(&cli.size)?;
    if let Some(parent) = cli.out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create output directory {}", parent.display()))?;
    }
    render_one(
        view,
        cli.theme,
        cli.accent,
        Appearance {
            font_size: cli.font_size,
            density: cli.density,
            zebra: !cli.no_zebra,
        },
        cli.visualizer,
        cli.settings_tab,
        width,
        height,
        &cli.out,
    )
}

/// Renders every view by re-invoking this binary once per view, so each gets a
/// fresh process and window.
fn run_all(cli: &Cli) -> anyhow::Result<()> {
    let dir = output_dir(&cli.out);
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("create output directory {}", dir.display()))?;
    let exe = std::env::current_exe().context("resolve current executable")?;
    for view in View::ALL {
        let out = dir.join(format!("{}.png", view.slug()));
        let status = Command::new(&exe)
            .arg("--view")
            .arg(view.slug())
            .arg("--theme")
            .arg(cli.theme.slug())
            .arg("--accent")
            .arg(cli.accent.to_config_str())
            .arg("--font-size")
            .arg(font_size_slug(cli.font_size))
            .arg("--density")
            .arg(density_slug(cli.density))
            .args(if cli.no_zebra {
                vec!["--no-zebra"]
            } else {
                vec![]
            })
            .args(
                cli.settings_tab
                    .iter()
                    .flat_map(|tab| ["--settings-tab", tab.slug()]),
            )
            .args(
                cli.visualizer
                    .iter()
                    .flat_map(|mode| ["--visualizer", mode.slug()]),
            )
            .arg("--size")
            .arg(&cli.size)
            .arg("--out")
            .arg(&out)
            .status()
            .with_context(|| format!("spawn {}", exe.display()))?;
        if !status.success() {
            bail!(
                "emusic-portable-shot: rendering {} failed ({status})",
                view.slug()
            );
        }
    }
    Ok(())
}

/// The `--out` directory for `--all` (a `.png` `--out` uses its parent).
fn output_dir(out: &Path) -> PathBuf {
    if out.extension().is_some() {
        out.parent()
            .unwrap_or(Path::new("target/shots"))
            .to_path_buf()
    } else {
        out.to_path_buf()
    }
}

/// Parses `--accent`: a preset name or `#rrggbb`.
fn parse_accent(s: &str) -> std::result::Result<Accent, String> {
    Accent::parse(s)
        .ok_or_else(|| format!("invalid accent {s:?}: expected a preset name or #rrggbb"))
}

/// Parses `--visualizer`: a mode slug.
fn parse_visualizer(s: &str) -> std::result::Result<VisualizerMode, String> {
    VisualizerMode::from_slug(s).ok_or_else(|| format!("invalid visualizer {s:?}"))
}

/// Parses `--font-size`: one of the [`FontSize`] labels, case-insensitively.
fn parse_font_size(s: &str) -> std::result::Result<FontSize, String> {
    FontSize::ALL
        .into_iter()
        .find(|size| size.label().eq_ignore_ascii_case(s))
        .ok_or_else(|| {
            let known: Vec<&str> = FontSize::ALL.iter().map(|size| size.label()).collect();
            format!(
                "invalid font size {s:?}; expected one of: {}",
                known.join(", ")
            )
        })
}

/// Parses `--density`: one of the [`Density`] labels, case-insensitively.
fn parse_density(s: &str) -> std::result::Result<Density, String> {
    Density::ALL
        .into_iter()
        .find(|density| density.label().eq_ignore_ascii_case(s))
        .ok_or_else(|| {
            let known: Vec<&str> = Density::ALL.iter().map(|density| density.label()).collect();
            format!(
                "invalid density {s:?}; expected one of: {}",
                known.join(", ")
            )
        })
}

/// Parses `--settings-tab`: one of the [`SettingsTab`] slugs.
fn parse_settings_tab(s: &str) -> std::result::Result<SettingsTab, String> {
    SettingsTab::ALL
        .into_iter()
        .find(|tab| tab.slug().eq_ignore_ascii_case(s))
        .ok_or_else(|| {
            let known: Vec<&str> = SettingsTab::ALL.iter().map(|tab| tab.slug()).collect();
            format!(
                "invalid settings tab {s:?}; expected one of: {}",
                known.join(", ")
            )
        })
}

/// The `--font-size` value to pass on to a child process.
fn font_size_slug(size: FontSize) -> &'static str {
    size.label()
}

/// The `--density` value to pass on to a child process.
fn density_slug(density: Density) -> &'static str {
    density.label()
}

/// Resolves a `--view` slug, listing the known ones on error.
fn parse_view(slug: &str) -> anyhow::Result<View> {
    View::from_slug(slug).ok_or_else(|| {
        let known: Vec<&str> = View::ALL.iter().map(|view| view.slug()).collect();
        anyhow!(
            "unknown --view {slug:?}; expected one of: {}",
            known.join(", ")
        )
    })
}

/// Runs the app for one view and writes its capture to `out`.
#[expect(clippy::too_many_arguments, reason = "one flag per CLI switch")]
fn render_one(
    view: View,
    theme: ThemeArg,
    accent: Accent,
    appearance: Appearance,
    visualizer: Option<VisualizerMode>,
    settings_tab: Option<SettingsTab>,
    width: f32,
    height: f32,
    out: &Path,
) -> anyhow::Result<()> {
    // Theme, view and appearance go through the config so the shell adopts
    // them exactly as it adopts a user's saved settings.
    let mut config = Config {
        theme: theme.shell(),
        accent,
        appearance,
        visualizer_enabled: visualizer.is_some(),
        visualizer: visualizer.unwrap_or_default(),
        last_view: view,
        ..Config::default()
    };
    if view == View::Visualization {
        // Mark a placeholder preset as the one showing, so the browser shot
        // shows its current-preset highlight (#338).
        config.projectm.last_preset = Some(std::path::PathBuf::from(
            "visualizations/presets/cream-of-the-crop/Dancer.milk",
        ));
    }
    let out = out.to_path_buf();
    let spec = window_spec(width, height);
    let backdrop = theme.backdrop();

    let backend: Rc<dyn Backend> = Rc::new(xui::xui_canvas::WinitBackend::new());
    run_app(backend, spec, move |ui| {
        let waker = WakerSlot::new();
        let backends = backend::build(true, waker.handle());
        let mut app = Win32App::new(
            ui,
            backends.library,
            backends.player,
            config,
            None,
            None,
            Vec::new(),
            waker,
            true,
        );
        if let Some(notice) = backends.notice {
            app.set_backend_notice(notice);
        }
        // Show the requested Settings tab (the tab strip is built from the
        // shell state when the layout is next installed).
        if let (View::Settings, Some(tab)) = (view, settings_tab) {
            ui.emit(Msg::Settings(SettingsMsg::SelectTab(tab)));
        }
        let timer = ui.set_timer(TICK_MS);
        ShotApp {
            app,
            out,
            backdrop,
            timer: (timer.0 != 0).then_some(timer),
            deadline: Instant::now() + SETTLE,
            done: false,
        }
    })
    .map_err(|error| anyhow!("xui: {error}"))
}

/// Wraps the real app: after the settle period it captures the window, writes
/// the PNG and quits, ending the run for this view. `Ui::close` only tears
/// down the window; on the `winit`-backed canvas backend the event loop keeps
/// running until `Ui::quit` ends it, unlike `win32ui`, where closing the
/// primary window ends the native message loop.
struct ShotApp {
    app: Win32App,
    out: PathBuf,
    backdrop: [u8; 3],
    timer: Option<TimerId>,
    deadline: Instant,
    done: bool,
}

impl App for ShotApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        self.app.update(msg, ui);
        if self.done || Instant::now() < self.deadline {
            return;
        }
        self.done = true;
        if let Some(id) = self.timer.take() {
            ui.kill_timer(id);
        }
        match capture(ui) {
            Ok(image) => match write_png(flatten(image, self.backdrop), &self.out) {
                Ok(()) => eprintln!("wrote {}", self.out.display()),
                Err(error) => eprintln!(
                    "emusic-portable-shot: failed to write {}: {error:#}",
                    self.out.display()
                ),
            },
            Err(error) => eprintln!("emusic-portable-shot: capture failed: {error:#}"),
        }
        ui.close();
        ui.quit();
    }
}

/// Captures the canvas backend's software-composited surface.
fn capture(ui: &Ui<Msg>) -> anyhow::Result<Image> {
    ui.capture().map_err(|error| anyhow!("{error}"))
}

/// Composites the image over an opaque `backdrop` colour, so shots never
/// contain transparent regions that viewers would paint arbitrarily. Returns
/// `(width, height, rgba pixels)`, ready for [`write_png`].
fn flatten(image: Image, backdrop: [u8; 3]) -> (u32, u32, Vec<u8>) {
    let (width, height) = image.size();
    let mut pixels = image.pixels().to_vec();
    for pixel in pixels.as_chunks_mut::<4>().0.iter_mut() {
        let alpha = u16::from(pixel[3]);
        for (channel, behind) in pixel.iter_mut().zip(backdrop) {
            let blended = u16::from(*channel) * alpha + u16::from(behind) * (255 - alpha);
            *channel = u8::try_from(blended / 255).unwrap_or(u8::MAX);
        }
        pixel[3] = u8::MAX;
    }
    (width, height, pixels)
}

/// Writes an RGBA image as a PNG, creating the parent directory if needed.
fn write_png((width, height, pixels): (u32, u32, Vec<u8>), path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    Ok(())
}

/// Parses `--size` as `<width>x<height>`.
fn parse_size(spec: &str) -> anyhow::Result<(f32, f32)> {
    let (width, height) = spec
        .split_once('x')
        .ok_or_else(|| anyhow!("invalid --size {spec:?}: expected <width>x<height>"))?;
    let width = width
        .parse()
        .with_context(|| format!("invalid --size width {width:?} in {spec:?}"))?;
    let height = height
        .parse()
        .with_context(|| format!("invalid --size height {height:?} in {spec:?}"))?;
    Ok((width, height))
}
