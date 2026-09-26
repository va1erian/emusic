//! Everything the window can ask the app to do, and the OS-shell mapping.

use emusic_platform::ShellAction;
use emusic_ui::library_api::StatsWindow;
use emusic_ui::state::{Accent, Command, ShortcutAction, View};
use emusic_ui::views::column_browser::Pane;

use crate::views::track_table::ContextAction;

/// Everything the window can ask the app to do.
pub enum Msg {
    /// A background worker (search, IPC, image decode) woke the UI.
    Wake,
    /// The window's client area changed size.
    Resize,
    /// The shell's repaint timer fired.
    Timer,
    /// Apply a command from the menu bar.
    Dispatch(Command),
    /// Switch the central view (navigator row click).
    Navigate(View),
    /// The Folders tree selected a directory.
    FoldersSelect(String),
    /// The Folders view's "include subfolders" checkbox changed.
    FoldersSubfolders(bool),
    /// Play the Music view row (double-click / Enter).
    PlayRow(usize),
    /// Toggle the star of a track table row.
    ToggleStarRow(usize),
    /// Sort the Music view by a header column.
    SortColumn(usize),
    /// Open the context menu for a Music view row.
    ContextRow(usize),
    /// Run a track-table context-menu action.
    ContextAction(ContextAction),
    /// Open File -> Database info.
    DatabaseInfo,
    /// Show Help -> Keyboard shortcuts.
    KeyboardShortcuts,
    /// Go to Help -> About (the Settings view's About page).
    About,
    /// Pick a new accent colour from the Settings page (#40).
    SetAccent(Accent),
    /// The tag editor left a save request in its bridge (#376).
    TagEditorApply,
    /// Start a shuffled playback over the Music view's currently visible
    /// tracks (its header's "Shuffle all" button, #242).
    MusicShuffleAll,
    /// A column-browser pane's selection changed (the rows now selected).
    BrowserRow { pane: Pane, rows: Vec<usize> },
    /// Shuffle-play the artist/genre of the activated name+counts row
    /// (Artists/Genres; the portable context menu is #376's).
    NameCountShuffle(usize),
    /// The Most Played view's time-window selector changed.
    MostPlayed(StatsWindow),
    /// Ask to clear the whole play history; shows the confirmation dialog.
    HistoryClear,
    /// The user confirmed clearing the whole play history.
    HistoryClearConfirmed,
    /// A keyboard shortcut from the central `SHORTCUTS` table fired.
    Shortcut(ShortcutAction),
    /// A transport action the OS shell asked for (#320, #321).
    Shell(ShellAction),
    /// Minimize the window (a portable window button, non-Windows chrome).
    Minimize,
    /// Toggle the window between maximized and restored (non-Windows chrome).
    ToggleMaximize,
    /// Close the window and exit.
    Quit,
}

impl From<ShellAction> for Msg {
    fn from(action: ShellAction) -> Msg {
        Msg::Shell(action)
    }
}

/// Maps an OS shell action to the shared transport command.
pub(super) fn shell_command(action: ShellAction) -> Option<Command> {
    Some(match action {
        ShellAction::PlayPause => Command::PlayerPlayPause,
        ShellAction::Next => Command::PlayerNext,
        ShellAction::Previous => Command::PlayerPrevious,
        ShellAction::Stop => Command::PlayerStop,
        ShellAction::Seek(position) => Command::PlayerSeek(position),
    })
}
