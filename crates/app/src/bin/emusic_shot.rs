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

use emusic::app::EguiApp;
use emusic::config::Config;
use emusic::library_api::{Candidate, LibraryDataSource};
use emusic::mock::{MockLibrary, MockPlayer};
use emusic::state::{Accent, SettingsTab, Theme, View, VisualizerMode};
use emusic::tag_editor::AutoTagState;

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

    /// Render against a populated mock library with an auto-tag lookup in
    /// flight, so the status bar's lookup line and Cancel button can be
    /// screenshotted (#210). Takes precedence over `--empty`/`--scanning`.
    #[arg(long)]
    auto_tagging: bool,

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

    /// Opens the single-track tag editor for the first track before rendering
    /// (#172), so the dialog can be screenshotted headlessly.
    #[arg(long)]
    tag_editor: bool,

    /// Auto-tag lookup state to render in the open tag editor (#209):
    /// `searching`, `matches`, `no-match` or `failed`. Requires `--tag-editor`.
    #[arg(long, value_enum)]
    tag_editor_state: Option<AutoTagArg>,

    /// Opens the File -> Database info dialog before rendering (#193), so it
    /// can be screenshotted headlessly.
    #[arg(long)]
    database_info: bool,

    /// Visualizer strip mode to render (#25): `spectrum`, `oscilloscope`,
    /// `milkdrop` or `off`. Omitted, the strip stays hidden, matching the
    /// app's default.
    #[arg(long, value_parser = parse_visualizer)]
    visualizer: Option<VisualizerMode>,

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

/// The tag editor's auto-tag lookup state to render (#209).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum AutoTagArg {
    Searching,
    Matches,
    NoMatch,
    Failed,
}

impl AutoTagArg {
    /// The dialog state to render, with canned candidates for `Matches`.
    fn into_state(self) -> AutoTagState {
        match self {
            Self::Searching => AutoTagState::Searching,
            Self::Matches => AutoTagState::Matches(demo_candidates()),
            Self::NoMatch => AutoTagState::NoMatch,
            Self::Failed => AutoTagState::Failed(
                "Could not reach MusicBrainz; check your connection".to_string(),
            ),
        }
    }
}

/// Canned candidates for the `--tag-editor-state matches` screenshot.
fn demo_candidates() -> Vec<Candidate> {
    vec![
        Candidate {
            title: Some("Around the World".to_string()),
            artist: Some("Daft Punk".to_string()),
            album: Some("Homework".to_string()),
            album_artist: Some("Daft Punk".to_string()),
            year: Some(1997),
            track_no: Some(5),
            disc_no: Some(1),
            score: 0.96,
            ..Default::default()
        },
        Candidate {
            title: Some("Around the World (radio edit)".to_string()),
            artist: Some("Daft Punk".to_string()),
            year: Some(1997),
            score: 0.61,
            ..Default::default()
        },
    ]
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
    VisualizerMode::from_slug(s).ok_or_else(|| {
        format!("invalid visualizer {s:?}: expected spectrum, oscilloscope, milkdrop or off")
    })
}

fn parse_settings_tab(s: &str) -> Result<SettingsTab, String> {
    SettingsTab::from_slug(s).ok_or_else(|| {
        format!("invalid settings tab {s:?}: expected library, appearance, associations, playback or about")
    })
}

fn main() {
    let cli = Cli::parse();
    let (width, height) = parse_size(&cli.size);
    let mode = LibraryMode::from_flags(cli.empty, cli.scanning, cli.auto_tagging);

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
            tag_editor: cli.tag_editor,
            tag_editor_state: cli.tag_editor_state,
            database_info: cli.database_info,
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
        tag_editor: cli.tag_editor,
        tag_editor_state: cli.tag_editor_state,
        database_info: cli.database_info,
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
/// the first-run mid-scan state, or a populated library with an auto-tag
/// lookup in flight (#210).
#[derive(Clone, Copy)]
enum LibraryMode {
    Populated,
    Empty,
    Scanning,
    AutoTagging,
}

impl LibraryMode {
    fn from_flags(empty: bool, scanning: bool, auto_tagging: bool) -> Self {
        if auto_tagging {
            Self::AutoTagging
        } else if scanning {
            Self::Scanning
        } else if empty {
            Self::Empty
        } else {
            Self::Populated
        }
    }

    fn build(self) -> (MockLibrary, MockPlayer) {
        match self {
            Self::Populated | Self::AutoTagging => {
                let library = match self {
                    Self::AutoTagging => MockLibrary::auto_tagging(),
                    _ => MockLibrary::new(),
                };
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
    /// Visualizer strip mode to render; `None` leaves the strip hidden, as in
    /// a default app run.
    visualizer: Option<VisualizerMode>,
    /// Open the track Properties dialog before rendering (#136).
    properties: bool,
    /// Open the single-track tag editor before rendering (#172).
    tag_editor: bool,
    /// The tag editor's auto-tag lookup state to render (#209).
    tag_editor_state: Option<AutoTagArg>,
    /// Open the File -> Database info dialog before rendering (#193).
    database_info: bool,
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
        // The strip is opt-in, so it is only shown when `--visualizer` asked
        // for a mode; otherwise the shot matches a default app run.
        visualizer_enabled: args.visualizer.is_some(),
        visualizer: args.visualizer.unwrap_or(defaults.visualizer),
        library_folders: synthetic_folders(args.folders),
        ..defaults
    };

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(width, height))
        .build_eframe(|cc| {
            let (library, player) = args.mode.build();
            EguiApp::with_config(cc, Box::new(library), Box::new(player), config)
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
    if args.tag_editor {
        harness.state_mut().open_tag_editor();
    }
    if let Some(state) = args.tag_editor_state {
        harness
            .state_mut()
            .set_tag_editor_auto_tag(state.into_state());
    }
    if args.database_info {
        harness.state_mut().open_database_info();
    }
    // A single step is enough for a static screenshot; `Harness::run` would
    // wait for the UI to go idle, which it never does here because the
    // shell's repaint policy (#6) keeps requesting frames while "playing".
    // When a search query is active, the match runs on a background thread
    // (#22): give it real wall-clock time to answer, then run a couple more
    // steps so the UI thread polls and renders the result rather than a
    // still-empty "pending" frame.
    harness.run_steps(1);
    if args.search.query.is_some()
        || args.search.popup
        || args.properties
        || args.tag_editor
        || args.database_info
    {
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
