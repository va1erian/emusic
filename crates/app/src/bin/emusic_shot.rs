#![forbid(unsafe_code)]

//! Headless screenshot tool for the app (#118), scaled down for the portable
//! runtime.
//!
//! Runs the real [`Win32App`](emusic::app::Win32App) against the deterministic
//! mock backends and writes one PNG of the main window. Capture uses
//! `Windows.Graphics.Capture` through `xui_win32::capture_hwnd` (the `wgc`
//! feature), so it reads the DWM-composited surface without raising the window.
//!
//! `--dialog properties|tags|database` opens the respective secondary dialog
//! (#376) and captures *that* window instead, through the portable
//! `WindowHandle::capture` path.
//!
//! ```text
//! cargo run -p emusic --features shot --bin emusic-shot -- --view music --out shot.png
//! cargo run -p emusic --features shot --bin emusic-shot -- --dialog properties --out props.png
//! ```

use std::cell::RefCell;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, anyhow};
use clap::Parser;
use xui::xui_core::app::{App, Ui, WindowHandle};
use xui::xui_core::backend::{Backend, PlatformSpec, TimerId};
use xui_win32::Hwnd;
use xui_win32::capture::capture_hwnd;

use emusic_ui::config::Config;
use emusic_ui::library_api::TrackInfo;
use emusic_ui::state::{Accent, SettingsTab, Theme, View};
use emusic_ui::tag_editor::{Status, TagEditorState};
use emusic_ui::waker::WakerSlot;

use emusic::app::{Msg, Win32App};
use emusic::dialogs::{database_info, properties, tag_editor};
use emusic::window::window_spec;

/// How long the window is left to settle before the capture.
const SETTLE: Duration = Duration::from_millis(350);
/// The tool's own tick interval, so it keeps driving while the shell is idle.
const TICK_MS: u32 = 40;

#[derive(Parser, Debug)]
#[command(name = "emusic-shot")]
struct Cli {
    /// View to show, e.g. `music`. Ignored with `--all`.
    #[arg(long)]
    view: Option<String>,

    /// Settings sub-page to show with `--view settings`, e.g. `visualization`.
    #[arg(long)]
    settings_tab: Option<String>,

    /// Render every view into `--out`'s directory, one fresh process each.
    #[arg(long)]
    all: bool,

    /// Capture a dialog window instead of a view.
    #[arg(long, value_enum)]
    dialog: Option<DialogArg>,

    #[arg(long, value_enum, default_value = "dark")]
    theme: ThemeArg,

    /// Accent colour: a preset name or `#rrggbb`.
    #[arg(long, value_parser = parse_accent, default_value = "orange")]
    accent: Accent,

    /// `<width>x<height>`, e.g. `1280x800`.
    #[arg(long, default_value = "1280x800")]
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

/// The dialog the `--dialog` flag captures.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum DialogArg {
    Properties,
    Tags,
    Database,
}

impl ThemeArg {
    fn shell(self) -> Theme {
        match self {
            Self::Dark => Theme::Dark,
            Self::Light => Theme::Light,
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let (width, height) = parse_size(&cli.size)?;

    if cli.all {
        return run_all(&cli);
    }

    if let Some(parent) = cli.out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create output directory {}", parent.display()))?;
    }

    if let Some(dialog) = cli.dialog {
        return render_dialog(dialog, cli.theme, cli.accent, width, height, &cli.out);
    }

    let view = match cli.view.as_deref() {
        Some(slug) => parse_view(slug)?,
        None => View::Music,
    };
    let settings_tab = match cli.settings_tab.as_deref() {
        Some(slug) => Some(parse_settings_tab(slug)?),
        None => None,
    };
    render_one(
        view,
        settings_tab,
        cli.theme,
        cli.accent,
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
            .arg("--size")
            .arg(&cli.size)
            .arg("--out")
            .arg(&out)
            .status()
            .with_context(|| format!("spawn {}", exe.display()))?;
        if !status.success() {
            anyhow::bail!("emusic-shot: rendering {} failed ({status})", view.slug());
        }
    }
    // The Settings view has one page per tab; capture each so every page gets a
    // reference screenshot (`settings-<slug>.png`). The default tab is already
    // `settings.png` from the view loop above, so it is not written twice.
    for tab in SettingsTab::ALL.into_iter().skip(1) {
        let out = dir.join(format!("settings-{}.png", tab.slug()));
        let status = Command::new(&exe)
            .arg("--view")
            .arg("settings")
            .arg("--settings-tab")
            .arg(tab.slug())
            .arg("--theme")
            .arg(cli.theme.slug())
            .arg("--accent")
            .arg(cli.accent.to_config_str())
            .arg("--size")
            .arg(&cli.size)
            .arg("--out")
            .arg(&out)
            .status()
            .with_context(|| format!("spawn {}", exe.display()))?;
        if !status.success() {
            anyhow::bail!(
                "emusic-shot: rendering settings-{} failed ({status})",
                tab.slug()
            );
        }
    }
    Ok(())
}

/// The `--out` directory for `--all` (a `.png` `--out` uses its parent).
fn output_dir(out: &std::path::Path) -> PathBuf {
    if out.extension().is_some() {
        out.parent().unwrap_or(out).to_path_buf()
    } else {
        out.to_path_buf()
    }
}

/// Parses `--size`: `<width>x<height>`.
fn parse_size(value: &str) -> anyhow::Result<(f32, f32)> {
    let (width, height) = value
        .split_once('x')
        .ok_or_else(|| anyhow!("invalid --size {value:?}: expected <width>x<height>"))?;
    let width = width.parse::<f32>().context("parse --size width")?;
    let height = height.parse::<f32>().context("parse --size height")?;
    Ok((width, height))
}

/// Parses `--accent`: a preset name or `#rrggbb`.
fn parse_accent(s: &str) -> std::result::Result<Accent, String> {
    Accent::parse(s)
        .ok_or_else(|| format!("invalid accent {s:?}: expected a preset name or #rrggbb"))
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

/// Resolves a `--settings-tab` slug, listing the known ones on error.
fn parse_settings_tab(slug: &str) -> anyhow::Result<SettingsTab> {
    SettingsTab::from_slug(slug).ok_or_else(|| {
        let known: Vec<&str> = SettingsTab::ALL.iter().map(|tab| tab.slug()).collect();
        anyhow!(
            "unknown --settings-tab {slug:?}; expected one of: {}",
            known.join(", ")
        )
    })
}

/// Runs the app for one view and writes its capture to `out`.
fn render_one(
    view: View,
    settings_tab: Option<SettingsTab>,
    theme: ThemeArg,
    accent: Accent,
    width: f32,
    height: f32,
    out: &std::path::Path,
) -> anyhow::Result<()> {
    let mut config = Config {
        theme: theme.shell(),
        accent,
        last_view: view,
        ..Config::default()
    };
    if view == View::Visualization {
        // Mark a placeholder preset as the one showing, so the browser shot
        // shows its current-preset selection (#338).
        config.projectm.last_preset = Some(PathBuf::from(
            "visualizations/presets/cream-of-the-crop/Dancer.milk",
        ));
    }
    let out = out.to_path_buf();
    let backend = Rc::new(xui_win32::Win32Backend::new());
    let handle_backend = Rc::clone(&backend);
    let backend: Rc<dyn Backend> = backend;
    xui::xui_core::run_app(backend, window_spec(width, height), move |ui| {
        let hwnd = handle_backend.window_hwnd(ui.window());
        let waker = WakerSlot::new();
        let backends = emusic_ui::backend::build(true, waker.handle());
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
        let timer = ui.set_timer(TICK_MS);
        ShotApp {
            app,
            hwnd,
            out: out.clone(),
            settings_tab,
            applied_tab: false,
            timer: Some(timer),
            deadline: Instant::now() + SETTLE,
            done: false,
        }
    })
    .map_err(|error| anyhow!("{error}"))
}

/// Wraps the real app: after the settle period it captures the window, writes
/// the PNG and closes.
struct ShotApp {
    app: Win32App,
    hwnd: Option<Hwnd>,
    out: PathBuf,
    /// The Settings sub-page to select before the capture, if any.
    settings_tab: Option<SettingsTab>,
    /// Whether that sub-page has been applied yet.
    applied_tab: bool,
    timer: Option<TimerId>,
    deadline: Instant,
    done: bool,
}

impl App for ShotApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
        if !self.applied_tab {
            self.applied_tab = true;
            if let Some(tab) = self.settings_tab {
                self.app.update(
                    Msg::Settings(emusic::views::settings::SettingsMsg::SelectTab(tab)),
                    ui,
                );
            }
        }
        self.app.update(msg, ui);
        if self.done || Instant::now() < self.deadline {
            return;
        }
        self.done = true;
        if let Some(id) = self.timer.take() {
            ui.kill_timer(id);
        }
        match self.hwnd {
            Some(hwnd) => match capture_hwnd(hwnd) {
                Ok(image) => match write_png(&image.pixels, image.width, image.height, &self.out) {
                    Ok(()) => eprintln!("wrote {}", self.out.display()),
                    Err(error) => eprintln!("emusic-shot: write {}: {error:#}", self.out.display()),
                },
                Err(error) => eprintln!("emusic-shot: capture failed: {error}"),
            },
            None => eprintln!("emusic-shot: the window has no native handle to capture"),
        }
        ui.close();
        // `ui.close()` only closes the window; the run loop still has to be
        // told to stop, or the process hangs after the capture.
        ui.quit();
    }
}

/// Runs the app for one dialog and writes its capture to `out`.
fn render_dialog(
    dialog: DialogArg,
    theme: ThemeArg,
    accent: Accent,
    width: f32,
    height: f32,
    out: &std::path::Path,
) -> anyhow::Result<()> {
    let backend: Rc<dyn Backend> = Rc::new(xui_win32::Win32Backend::new());
    let spec = window_spec(width, height);
    let out = out.to_path_buf();
    let waker = WakerSlot::new();
    let backends = emusic_ui::backend::build(true, waker.handle());
    let track = mock_track();
    let theme = theme.shell();

    match dialog {
        DialogArg::Properties => run_dialog(backend, spec, out, theme, accent, move |ui| {
            properties::open(ui, &track)
        }),
        DialogArg::Tags => {
            let state = TagEditorState::new(&track);
            run_dialog(backend, spec, out, theme, accent, move |ui| {
                let bridge = Rc::new(RefCell::new(tag_editor::Bridge::new(Status::Editing)));
                tag_editor::open(ui, &state, bridge, ui.proxy())
            })
        }
        DialogArg::Database => {
            let library = backends.library;
            run_dialog(backend, spec, out, theme, accent, move |ui| {
                database_info::open(ui, library.as_ref())
            })
        }
    }
}

/// A deterministic track the dialog shots render.
fn mock_track() -> TrackInfo {
    TrackInfo {
        id: 1,
        title: "Yellow Ledbetter".to_owned(),
        artist: "Pearl Jam".to_owned(),
        album: "Jeremy".to_owned(),
        album_artist: "Pearl Jam".to_owned(),
        genre: "Rock".to_owned(),
        year: Some(1992),
        track_no: Some(4),
        disc_no: Some(1),
        composer: "Eddie Vedder".to_owned(),
        duration: Duration::from_secs(301),
        path: r"C:\Music\Pearl Jam\Jeremy\04 - Yellow Ledbetter.flac".to_owned(),
        format: "flac".to_owned(),
        codec: "FLAC".to_owned(),
        bitrate: Some(920),
        sample_rate: Some(44_100),
        bit_depth: Some(16),
        channels: Some(2),
        play_count: 12,
        last_played_minutes_ago: Some(90),
        starred: true,
        ..TrackInfo::default()
    }
}

/// Opens one dialog window on a throwaway main window, captures it after the
/// settle period and quits.
fn run_dialog<H: 'static>(
    backend: Rc<dyn Backend>,
    spec: PlatformSpec,
    out: PathBuf,
    theme: Theme,
    accent: Accent,
    open: impl FnOnce(&Ui<Msg>) -> xui::xui_core::backend::Result<WindowHandle<H>> + 'static,
) -> anyhow::Result<()> {
    xui::xui_core::run_app(backend, spec, move |ui| {
        // The dialog inherits this window's theme at the moment it opens.
        ui.set_theme(emusic::theme::app_theme(theme, accent));
        // The throwaway main window only exists to own the tick timer, so its
        // `Msg::Timer` must be mapped or `DialogShotApp::update` is never called
        // and the process hangs on an open dialog. `Win32App::new` registers
        // this on the real main window; the shot's bare window has to do it.
        ui.on_timer(|_| Some(Msg::Timer));
        let handle = match open(ui) {
            Ok(handle) => Some(handle),
            Err(error) => {
                eprintln!("emusic-shot: could not open the dialog window: {error}");
                None
            }
        };
        let timer = ui.set_timer(TICK_MS);
        DialogShotApp {
            handle,
            out,
            timer: Some(timer),
            deadline: Instant::now() + SETTLE,
            done: false,
        }
    })
    .map_err(|error| anyhow!("{error}"))
}

/// Captures a secondary dialog window after the settle period.
struct DialogShotApp<H: 'static> {
    /// `None` when the dialog window could not be created; the app then just
    /// quits so a backend that cannot open it fails clearly instead of hanging.
    handle: Option<WindowHandle<H>>,
    out: PathBuf,
    timer: Option<TimerId>,
    deadline: Instant,
    done: bool,
}

impl<H: 'static> App for DialogShotApp<H> {
    type Msg = Msg;

    fn update(&mut self, _msg: Msg, ui: &mut Ui<Msg>) {
        if self.done || Instant::now() < self.deadline {
            return;
        }
        self.done = true;
        if let Some(id) = self.timer.take() {
            ui.kill_timer(id);
        }
        match self.handle.take() {
            Some(handle) => {
                match handle.capture() {
                    Ok(image) => {
                        match write_png(image.pixels(), image.width(), image.height(), &self.out) {
                            Ok(()) => eprintln!("wrote {}", self.out.display()),
                            Err(error) => {
                                eprintln!("emusic-shot: write {}: {error:#}", self.out.display())
                            }
                        }
                    }
                    Err(error) => eprintln!("emusic-shot: capture failed: {error}"),
                }
                handle.close();
            }
            None => eprintln!("emusic-shot: no dialog window was open to capture"),
        }
        ui.close();
        ui.quit();
    }
}

/// Writes straight-alpha RGBA pixels as a PNG.
fn write_png(pixels: &[u8], width: u32, height: u32, out: &std::path::Path) -> anyhow::Result<()> {
    let file = std::fs::File::create(out)?;
    let writer = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels)?;
    Ok(())
}
