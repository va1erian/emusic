//! Consumer demo for the external [`win32ui`](https://github.com/va1erian/win32ui)
//! crate (#105): a widget-layer window with a toolbar, a lazily-populated side
//! tree and a virtual (owner-data) track list, plus a status bar and a menu.
//!
//! Run with:
//!
//! ```text
//! cargo run -p win32ui-demo --example demo
//! ```
//!
//! Set `EMUSIC_WIN32UI_DEMO_AUTOCLOSE_MS=3000` to have the demo quit itself,
//! which makes a headless smoke run possible.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    use win32ui::prelude::*;
    use win32ui::{column, row};

    /// One row of mock library data. The `*_text` fields pre-format the numeric
    /// columns so owner-data requests never allocate.
    struct Track {
        title: String,
        artist: String,
        album: String,
        year_text: String,
        duration_text: String,
    }

    /// The app's messages: every widget event is mapped to one of these and
    /// delivered to [`Demo::update`], which is never re-entered.
    enum Msg {
        Quit,
        ToggleTheme,
        Play(usize),
        TreeSelect,
        AutoClose,
    }

    /// Lazily-populated side tree, standing in for the library navigator.
    /// Nodes are keyed by an `i64` node id.
    struct LibraryTree;

    impl TreeModel for LibraryTree {
        type Key = i64;

        fn children(&self, parent: Option<&i64>) -> Vec<Node<i64>> {
            match parent {
                None => vec![Node::branch(1, "Music"), Node::branch(2, "Playlists")],
                Some(&1) => vec![
                    Node::leaf(11, "Rock"),
                    Node::leaf(12, "Jazz"),
                    Node::leaf(13, "Electronic"),
                ],
                Some(&2) => vec![Node::leaf(21, "Favourites")],
                _ => Vec::new(),
            }
        }
    }

    /// Deterministic mock tracks, enough to exercise the virtual list.
    fn mock_tracks(count: usize) -> Vec<Track> {
        const TITLES: &[&str] = &["Aurora Fields", "Harbour Lights", "Static", "Reverie"];
        const ARTISTS: &[&str] = &["The Midnight Set", "Cassette Ghosts", "Vela"];
        const ALBUMS: &[&str] = &["First Light", "Signal", "Northern"];
        (0..count)
            .map(|index| {
                let seconds = 120 + (index % 240) as u32;
                Track {
                    title: format!("{} {}", TITLES[index % TITLES.len()], index + 1),
                    artist: ARTISTS[index % ARTISTS.len()].to_string(),
                    album: ALBUMS[index % ALBUMS.len()].to_string(),
                    year_text: (1950 + (index % 75)).to_string(),
                    duration_text: format!("{}:{:02}", seconds / 60, seconds % 60),
                }
            })
            .collect()
    }

    /// The app: owns the widgets (each keeps its child `HWND` alive).
    struct Demo {
        /// Held to keep the toolbar's `HWND` alive; its buttons raise `Msg`s
        /// directly, so the app never reads it.
        #[allow(dead_code)]
        toolbar: Toolbar<Msg>,
        tree: TreeView<i64, Msg>,
        list: ListView<Track, Msg>,
        status: StatusBar<Msg>,
        theme: Theme,
    }

    impl App for Demo {
        type Msg = Msg;

        fn update(&mut self, msg: Msg, ui: &mut Ui<Msg>) {
            match msg {
                Msg::Quit | Msg::AutoClose => ui.quit(),
                Msg::ToggleTheme => {
                    self.theme = if self.theme.is_dark {
                        Theme::light()
                    } else {
                        Theme::dark()
                    };
                    ui.set_theme(self.theme);
                }
                Msg::Play(row) => {
                    // The list is app-driven: selecting the row gives visual
                    // feedback (a playing-row highlight is a later, app-painted
                    // row style).
                    self.list.set_selection(&[row]);
                    self.status.set_text(0, &format!("Playing row {}", row + 1));
                }
                Msg::TreeSelect => {
                    let label = self
                        .tree
                        .selected()
                        .map_or_else(|| "nothing".to_string(), |id| format!("node {id}"));
                    self.status.set_text(0, &format!("Tree selection: {label}"));
                }
            }
        }
    }

    let result = win32ui::run_app(
        WindowSpec::new("emusic — win32ui demo")
            .size(dip(1100.0), dip(700.0))
            .theme(Theme::dark()),
        |ui| {
            let theme = ui.theme();
            let toolbar = Toolbar::new(
                ui,
                vec![
                    ToolbarItem::new("Play").on_click(|| Some(Msg::Play(0))),
                    ToolbarItem::new("Theme").on_click(|| Some(Msg::ToggleTheme)),
                ],
            )
            .expect("toolbar");

            let tree = TreeView::new(ui, LibraryTree)
                .expect("tree")
                .on_select(|_| Some(Msg::TreeSelect));

            // A virtual list: 20k rows are never materialised as controls.
            let list = ListView::new(ui)
                .expect("list")
                .column("Title", Fill, |track: &Track| track.title.as_str())
                .column("Artist", dip(180.0), |track: &Track| track.artist.as_str())
                .column("Album", dip(180.0), |track: &Track| track.album.as_str())
                .column_right("Year", dip(60.0), |track: &Track| track.year_text.as_str())
                .column_right("Time", dip(64.0), |track: &Track| {
                    track.duration_text.as_str()
                })
                .multi_select(true)
                .on_activate(|row| Some(Msg::Play(row)));
            list.set_model(mock_tracks(20_000));

            let status = StatusBar::new(ui).expect("status");
            status.set_text(0, "Ready — 20,000 tracks");

            ui.set_menu_bar(
                Menu::new()
                    .item("&Play", None, || Msg::Play(0))
                    .checked_item("&Dark theme", None, true, || Msg::ToggleTheme)
                    .separator()
                    .item("&Quit", Shortcut::ctrl(Key::Q), || Msg::Quit),
            );
            ui.accelerator(Shortcut::ctrl(Key::Q), || Some(Msg::Quit));
            ui.accelerator(Shortcut::ctrl(Key::T), || Some(Msg::ToggleTheme));

            ui.set_layout(column![
                toolbar,
                row![tree.width(dip(220.0)), list].fill(1),
                status
            ]);

            // Headless smoke runs: quit after a moment so the demo can be run
            // without a user to close it.
            let auto_close = std::env::var("EMUSIC_WIN32UI_DEMO_AUTOCLOSE_MS")
                .ok()
                .and_then(|millis| millis.parse().ok());
            if let Some(millis) = auto_close
                && let Ok(id) = ui.set_timer(millis)
            {
                ui.on_timer(move |fired| (fired == id).then_some(Msg::AutoClose));
            }

            Demo {
                toolbar,
                tree,
                list,
                status,
                theme,
            }
        },
    );

    if let Err(error) = result {
        eprintln!("win32ui: {error}");
    }
}

#[cfg(not(windows))]
fn main() {}
