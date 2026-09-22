#![forbid(unsafe_code)]

//! Headless screenshot tool (#32): renders the app (always with mock data)
//! off-screen via `egui_kittest`'s wgpu snapshot rendering (falls back to a
//! software adapter such as WARP where no GPU is available) and writes a
//! PNG. Kept as a separate binary, gated behind the `shot` feature, so
//! `egui_kittest`/wgpu never end up in the real `emusic.exe` dependency
//! tree.
//!
//! ```text
//! cargo run -p emusic --features shot --bin emusic-shot -- \
//!     --view music --size 1280x800 --theme dark --out target/shots/music.png
//! cargo run -p emusic --features shot --bin emusic-shot -- \
//!     --view music --accent blue --out target/shots/accent.png
//! cargo run -p emusic --features shot --bin emusic-shot -- --all
//! ```

use std::path::{Path, PathBuf};

use clap::Parser;
use eframe::egui;
use egui_kittest::Harness;

use emusic::app::App;
use emusic::config::Config;
use emusic::library_api::LibraryDataSource;
use emusic::mock::{MockLibrary, MockPlayer};
use emusic::state::{Accent, Theme, View};

#[derive(Parser, Debug)]
#[command(name = "emusic-shot")]
struct Cli {
    /// View to render, e.g. `music`, `albums`, `now-playing`. Ignored if
    /// `--all` is set.
    #[arg(long)]
    view: Option<String>,

    /// Render every view into `--out`'s directory, one PNG per view.
    #[arg(long)]
    all: bool,

    /// Render against an empty mock library (shows the first-run empty state).
    #[arg(long)]
    empty: bool,

    /// Render against an empty mock library that is mid-scan (shows the
    /// first-run "building your music library" state). Takes precedence over
    /// `--empty`.
    #[arg(long)]
    scanning: bool,

    /// `<width>x<height>`, e.g. `1280x800`.
    #[arg(long, default_value = "1280x800")]
    size: String,

    #[arg(long, value_enum, default_value = "dark")]
    theme: ThemeArg,

    /// Accent colour: a preset name (`orange`, `blue`, `green`, `purple`,
    /// `red`, `teal`) or `#rrggbb`.
    #[arg(long, value_parser = parse_accent)]
    accent: Option<Accent>,

    /// Output PNG path (single view) or directory (`--all`).
    #[arg(long, default_value = "target/shots/shot.png")]
    out: PathBuf,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ThemeArg {
    Dark,
    Light,
}

fn parse_accent(s: &str) -> Result<Accent, String> {
    Accent::parse(s).ok_or_else(|| {
        format!(
            "invalid accent {s:?}: expected a preset name or #rrggbb \
             (presets: orange, blue, green, purple, red, teal)"
        )
    })
}

fn main() {
    let cli = Cli::parse();
    let (width, height) = parse_size(&cli.size);
    let mode = LibraryMode::from_flags(cli.empty, cli.scanning);

    if cli.all {
        let dir = if cli.out.extension().is_some() {
            cli.out
                .parent()
                .unwrap_or(Path::new("target/shots"))
                .to_path_buf()
        } else {
            cli.out.clone()
        };
        std::fs::create_dir_all(&dir).expect("create output directory");
        for view in View::ALL {
            let out = dir.join(format!("{}.png", view.slug()));
            render_one(view, width, height, cli.theme, cli.accent, mode, &out);
        }
        return;
    }

    let view = cli
        .view
        .as_deref()
        .and_then(View::from_slug)
        .unwrap_or(View::Music);
    if let Some(parent) = cli.out.parent() {
        std::fs::create_dir_all(parent).expect("create output directory");
    }
    render_one(view, width, height, cli.theme, cli.accent, mode, &cli.out);
}

/// Mock library to render: the populated default, the first-run empty state,
/// or the first-run mid-scan state.
#[derive(Clone, Copy)]
enum LibraryMode {
    Populated,
    Empty,
    Scanning,
}

impl LibraryMode {
    fn from_flags(empty: bool, scanning: bool) -> Self {
        if scanning {
            Self::Scanning
        } else if empty {
            Self::Empty
        } else {
            Self::Populated
        }
    }

    fn build(self) -> (MockLibrary, MockPlayer) {
        match self {
            Self::Populated => {
                let library = MockLibrary::new();
                let player = MockPlayer::playing_demo(&library.tracks()[0]);
                (library, player)
            }
            Self::Empty => (MockLibrary::empty(), MockPlayer::default()),
            Self::Scanning => (MockLibrary::scanning(), MockPlayer::default()),
        }
    }
}

fn render_one(
    view: View,
    width: f32,
    height: f32,
    theme: ThemeArg,
    accent: Option<Accent>,
    mode: LibraryMode,
    out: &Path,
) {
    // Theme/accent go through the config so the shell applies them the
    // same way it applies user settings.
    let defaults = Config::default();
    let config = Config {
        theme: match theme {
            ThemeArg::Dark => Theme::Dark,
            ThemeArg::Light => Theme::Light,
        },
        accent: accent.unwrap_or(defaults.accent),
        ..defaults
    };

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(width, height))
        .build_eframe(|cc| {
            let (library, player) = mode.build();
            App::with_config(cc, Box::new(library), Box::new(player), config)
        });

    harness.state_mut().set_view(view);
    // A single step is enough for a static screenshot; `Harness::run` would
    // wait for the UI to go idle, which it never does here because the
    // shell's repaint policy (#6) keeps requesting frames while "playing".
    harness.run_steps(1);

    let image = harness.render().expect("headless render failed");
    image.save(out).expect("write screenshot PNG");
    eprintln!("wrote {}", out.display());
}

fn parse_size(spec: &str) -> (f32, f32) {
    let (w, h) = spec.split_once('x').unwrap_or(("1280", "800"));
    (w.parse().unwrap_or(1280.0), h.parse().unwrap_or(800.0))
}
