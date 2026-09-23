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
use emusic::state::{Accent, SettingsTab, Theme, View, VisualizerMode};

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

    /// Pre-fills the top-bar search box with this query before rendering
    /// (#22), so a filtered Music view can be screenshotted headlessly.
    #[arg(long)]
    query: Option<String>,

    /// Opens the global search popup, pre-filled with `--query` (or empty),
    /// before rendering (#22).
    #[arg(long)]
    search_popup: bool,

    /// Opens the Music table's track Properties dialog for the first track
    /// before rendering (#136), so the dialog can be screenshotted headlessly.
    #[arg(long)]
    properties: bool,

    /// Visualizer strip mode to render (#25): `spectrum` (the default),
    /// `oscilloscope` or `off`.
    #[arg(long, value_parser = parse_visualizer, default_value = "spectrum")]
    visualizer: VisualizerMode,

    /// Pre-populate the config with this many synthetic library folders, so
    /// Settings → Library can be screenshotted with a long, scrollable list
    /// (#137).
    #[arg(long, default_value = "0")]
    folders: usize,

    /// Settings sub-page to select when rendering `--view settings` (#137):
    /// `library` (the default), `appearance` or `associations`.
    #[arg(long, value_parser = parse_settings_tab)]
    settings_tab: Option<SettingsTab>,
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

fn parse_visualizer(s: &str) -> Result<VisualizerMode, String> {
    VisualizerMode::from_slug(s)
        .ok_or_else(|| format!("invalid visualizer {s:?}: expected spectrum, oscilloscope or off"))
}

fn parse_settings_tab(s: &str) -> Result<SettingsTab, String> {
    SettingsTab::from_slug(s).ok_or_else(|| {
        format!("invalid settings tab {s:?}: expected library, appearance or associations")
    })
}

fn main() {
    let cli = Cli::parse();
    let (width, height) = parse_size(&cli.size);
    let mode = LibraryMode::from_flags(cli.empty, cli.scanning);

    let search = SearchArgs {
        query: cli.query.clone(),
        popup: cli.search_popup,
    };

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
        let args = RenderArgs {
            size: (width, height),
            theme: cli.theme,
            accent: cli.accent,
            mode,
            search: &search,
            visualizer: cli.visualizer,
            properties: cli.properties,
            folders: cli.folders,
            settings_tab: cli.settings_tab,
        };
        for view in View::ALL {
            let out = dir.join(format!("{}.png", view.slug()));
            render_one(view, &args, &out);
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
    let args = RenderArgs {
        size: (width, height),
        theme: cli.theme,
        accent: cli.accent,
        mode,
        search: &search,
        visualizer: cli.visualizer,
        properties: cli.properties,
        folders: cli.folders,
        settings_tab: cli.settings_tab,
    };
    render_one(view, &args, &cli.out);
}

/// Search state (#22) to apply before rendering: a top-bar query and/or the
/// global popup, pre-filled and left open.
#[derive(Clone, Default)]
struct SearchArgs {
    query: Option<String>,
    popup: bool,
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

/// Bundles the CLI-derived rendering settings so [`render_one`] stays a
/// small, single-purpose function.
struct RenderArgs<'a> {
    size: (f32, f32),
    theme: ThemeArg,
    accent: Option<Accent>,
    mode: LibraryMode,
    search: &'a SearchArgs,
    visualizer: VisualizerMode,
    /// Open the track Properties dialog before rendering (#136).
    properties: bool,
    folders: usize,
    settings_tab: Option<SettingsTab>,
}

fn render_one(view: View, args: &RenderArgs, out: &Path) {
    let (width, height) = args.size;
    // Theme/accent go through the config so the shell applies them the
    // same way it applies user settings.
    let defaults = Config::default();
    let config = Config {
        theme: match args.theme {
            ThemeArg::Dark => Theme::Dark,
            ThemeArg::Light => Theme::Light,
        },
        accent: args.accent.unwrap_or(defaults.accent),
        visualizer: args.visualizer,
        library_folders: synthetic_folders(args.folders),
        ..defaults
    };

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(width, height))
        .build_eframe(|cc| {
            let (library, player) = args.mode.build();
            App::with_config(cc, Box::new(library), Box::new(player), config)
        });

    harness.state_mut().set_view(view);
    if let Some(tab) = args.settings_tab {
        harness.state_mut().set_settings_tab(tab);
    }
    if let Some(query) = &args.search.query {
        harness.state_mut().set_search_query(query.clone());
    }
    if args.search.popup {
        harness
            .state_mut()
            .open_search_popup(args.search.query.clone().unwrap_or_default());
    }
    if args.properties {
        harness.state_mut().open_track_properties();
    }
    // A single step is enough for a static screenshot; `Harness::run` would
    // wait for the UI to go idle, which it never does here because the
    // shell's repaint policy (#6) keeps requesting frames while "playing".
    // When a search query is active, the match runs on a background thread
    // (#22): give it real wall-clock time to answer, then run a couple more
    // steps so the UI thread polls and renders the result rather than a
    // still-empty "pending" frame.
    harness.run_steps(1);
    if args.search.query.is_some() || args.search.popup || args.properties {
        std::thread::sleep(std::time::Duration::from_millis(200));
        harness.run_steps(2);
    } else if view == View::Settings {
        // The Settings folder list's scroll bar is sized from the previous
        // frame's content and fades in over a few frames, so run several
        // more steps before the screenshot (#137).
        harness.run_steps(8);
    }

    let image = harness.render().expect("headless render failed");
    image.save(out).expect("write screenshot PNG");
    eprintln!("wrote {}", out.display());
}

/// Synthetic library folders for `--folders N` (#137): a long, deterministic
/// list that exercises the Settings folder list's scroll area.
fn synthetic_folders(count: usize) -> Vec<PathBuf> {
    (0..count)
        .map(|i| PathBuf::from(format!("D:/Music/Library/album-{i:03}")))
        .collect()
}

fn parse_size(spec: &str) -> (f32, f32) {
    let (w, h) = spec.split_once('x').unwrap_or(("1280", "800"));
    (w.parse().unwrap_or(1280.0), h.parse().unwrap_or(800.0))
}
