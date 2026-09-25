#![forbid(unsafe_code)]

//! Headless screenshot tool for the Win32 frontend (#118).
//!
//! Runs the real [`Win32App`](emusic_win32::app::Win32App) against the
//! deterministic mock backend and writes one PNG per view. Capture uses
//! `win32ui`'s occlusion-proof `Windows.Graphics.Capture` path
//! (`Ui::capture_composited`, the `wgc` feature): it reads the DWM-composited
//! surface, so it includes the frame, caption buttons and any backdrop
//! material and is not sensitive to child-window paint timing. When that is
//! unavailable at runtime it falls back to `PrintWindow`.
//!
//! One window per process: `--all` re-invokes this binary once per view. A
//! second window created in the same process was captured before it painted
//! (the placeholder views came out blank), so each view gets a fresh process.
//!
//! Kept as a separate binary, gated behind the `shot` feature, so the PNG
//! encoder and the WinRT capture bindings never end up in the real
//! `emusic-win32.exe` dependency tree.
//!
//! ```text
//! cargo run -p emusic-win32 --features shot --bin emusic-win32-shot -- \
//!     --view music --theme dark --out target/shots/win32/music.png
//! cargo run -p emusic-win32 --features shot --bin emusic-win32-shot -- --all
//! ```

use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context as _, anyhow, bail};
use clap::Parser;
use win32ui::prelude::*;

use emusic_ui::backend;
use emusic_ui::config::Config;
use emusic_ui::state::{Accent, Theme, View, VisualizerMode};
use emusic_ui::waker::WakerSlot;

use emusic_win32::app::{Msg, Win32App};
use emusic_win32::theme::win32_theme;
use emusic_win32::window::window_spec;

/// How long the window is left to settle (create its controls) before the
/// capture. The capture itself waits for a composited frame, so this is only
/// a small head start, not a paint-timing guess.
const SETTLE: Duration = Duration::from_millis(300);
/// The tool's own tick interval, so it keeps driving even when the shell's
/// repaint timer is idle (a stopped player schedules no frame).
const TICK_MS: u32 = 40;

#[derive(Parser, Debug)]
#[command(name = "emusic-win32-shot")]
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

    /// Show the top-bar visualizer in this mode (`spectrum`, `oscilloscope`).
    #[arg(long, value_parser = parse_visualizer)]
    visualizer: Option<VisualizerMode>,

    /// `<width>x<height>`, e.g. `1280x800`.
    #[arg(long, default_value = "1280x800")]
    size: String,

    /// Output PNG path (single view) or directory (`--all`).
    #[arg(long, default_value = "target/shots/win32/shot.png")]
    out: PathBuf,
}

/// The CLI's theme spelling, mapped to the shell's and `win32ui`'s palettes.
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

    /// Opaque stand-in for the system backdrop material behind the title
    /// strip, which the compositor capture leaves transparent in dark mode.
    fn backdrop(self) -> [u8; 3] {
        match self {
            Self::Dark => [32, 32, 32],
            Self::Light => [243, 243, 243],
        }
    }

    fn win32(self, accent: Accent) -> win32ui::Theme {
        win32_theme(self.shell(), accent)
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
        cli.visualizer,
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
                "emusic-win32-shot: rendering {} failed ({status})",
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
            .unwrap_or(Path::new("target/shots/win32"))
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
fn render_one(
    view: View,
    theme: ThemeArg,
    accent: Accent,
    visualizer: Option<VisualizerMode>,
    width: f32,
    height: f32,
    out: &Path,
) -> anyhow::Result<()> {
    // Theme and view go through the config so the shell adopts them exactly as
    // it adopts a user's saved settings.
    let config = Config {
        theme: theme.shell(),
        accent,
        visualizer_enabled: visualizer.is_some(),
        visualizer: visualizer.unwrap_or_default(),
        last_view: view,
        ..Config::default()
    };
    let out = out.to_path_buf();
    let spec = window_spec(width, height, theme.win32(accent));

    win32ui::run_app(spec, move |ui| {
        let waker = WakerSlot::new();
        let backends = backend::build(true, waker.handle());
        let mut app = Win32App::new(
            ui,
            backends.library,
            backends.player,
            config,
            None,
            None,
            None,
            waker,
        );
        if let Some(notice) = backends.notice {
            app.set_backend_notice(notice);
        }
        let timer = ui.set_timer(TICK_MS).ok();
        ShotApp {
            app,
            out,
            backdrop: theme.backdrop(),
            timer,
            deadline: Instant::now() + SETTLE,
            done: false,
        }
    })
    .map_err(|error| anyhow!("{error}"))
}

/// Wraps the real app: after the settle period it captures the window, writes
/// the PNG and closes, ending the run for this view.
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
            Ok(mut image) => match write_png(flatten(&mut image, self.backdrop), &self.out) {
                Ok(()) => eprintln!("wrote {}", self.out.display()),
                Err(error) => eprintln!(
                    "emusic-win32-shot: failed to write {}: {error:#}",
                    self.out.display()
                ),
            },
            Err(error) => eprintln!("emusic-win32-shot: capture failed: {error:#}"),
        }
        ui.close();
    }
}

/// Captures the window's DWM-composited surface, falling back to `PrintWindow`
/// where `Windows.Graphics.Capture` is unavailable.
fn capture(ui: &Ui<Msg>) -> anyhow::Result<RgbaImage> {
    match ui.capture_composited() {
        Ok(image) => Ok(image),
        Err(error) => {
            eprintln!(
                "emusic-win32-shot: composited capture unavailable ({error}); \
                 falling back to PrintWindow"
            );
            ui.capture().map_err(|error| anyhow!("{error}"))
        }
    }
}

/// Composites the image over an opaque `backdrop` colour, so shots never
/// contain transparent regions that viewers would paint arbitrarily.
fn flatten(image: &mut RgbaImage, backdrop: [u8; 3]) -> &RgbaImage {
    for pixel in image.pixels.as_chunks_mut::<4>().0.iter_mut() {
        let alpha = u16::from(pixel[3]);
        for (channel, behind) in pixel.iter_mut().zip(backdrop) {
            let blended = u16::from(*channel) * alpha + u16::from(behind) * (255 - alpha);
            *channel = u8::try_from(blended / 255).unwrap_or(u8::MAX);
        }
        pixel[3] = u8::MAX;
    }
    image
}

/// Writes an RGBA image as a PNG, creating the parent directory if needed.
fn write_png(image: &RgbaImage, path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
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
