#![forbid(unsafe_code)]

//! Headless screenshot tool for the app (#118), scaled down for the portable
//! runtime.
//!
//! Runs the real [`Win32App`](emusic::app::Win32App) against the deterministic
//! mock backends and writes one PNG of the main window. Capture uses
//! `Windows.Graphics.Capture` through `xui_win32::capture_hwnd` (the `wgc`
//! feature), so it reads the DWM-composited surface without raising the window.
//!
//! The multi-window dialog shots and the per-appearance flags of the original
//! tool are not ported yet: the portable `xui_core` runtime has no second
//! window and the dialogs are #376's, so this tool now captures the main window
//! only (follow-up to #370).
//!
//! ```text
//! cargo run -p emusic --features shot --bin emusic-shot -- --view music --out shot.png
//! ```

use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, anyhow};
use clap::Parser;
use xui::xui_core::app::{App, Ui};
use xui::xui_core::backend::{Backend, TimerId};
use xui_win32::Hwnd;
use xui_win32::capture::capture_hwnd;

use emusic_ui::config::Config;
use emusic_ui::state::{Accent, Theme, View};
use emusic_ui::waker::WakerSlot;

use emusic::app::{Msg, Win32App};
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

    /// Render every view into `--out`'s directory, one fresh process each.
    #[arg(long)]
    all: bool,

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

    let view = match cli.view.as_deref() {
        Some(slug) => parse_view(slug)?,
        None => View::Music,
    };
    if let Some(parent) = cli.out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create output directory {}", parent.display()))?;
    }
    render_one(view, cli.theme, cli.accent, width, height, &cli.out)
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

/// Runs the app for one view and writes its capture to `out`.
fn render_one(
    view: View,
    theme: ThemeArg,
    accent: Accent,
    width: f32,
    height: f32,
    out: &std::path::Path,
) -> anyhow::Result<()> {
    let config = Config {
        theme: theme.shell(),
        accent,
        last_view: view,
        ..Config::default()
    };
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
        );
        if let Some(notice) = backends.notice {
            app.set_backend_notice(notice);
        }
        let timer = ui.set_timer(TICK_MS);
        ShotApp {
            app,
            hwnd,
            out: out.clone(),
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
