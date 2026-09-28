# Playlist support: plan

Status: partially implemented. Phases 1 and 2 (storage, m3u export, the `emusic-ui` layer) landed in #474; the frontends, drag and drop and m3u import are still to do.

## Requirements

1. A **Playlists** section in the left navigator, below **Activity**. A `+` button at the right of the section label creates a playlist.
2. Drag one or more tracks from a track list onto a playlist to add them.
3. Reorder items inside a playlist.
4. Right-click a track selection in a playlist to delete it. Right-click a playlist in the navigator to delete it.
5. Export a playlist as `.m3u`.
6. Play a playlist in linear or random (shuffle) order.

## Architecture (follows docs/frontends.md: logic in `emusic-ui`, frontends stay thin)

### 1. Storage: `crates/core`, `crates/library`
- `emusic_core::PlaylistId(pub i64)`.
- Migration **v7** in `store/schema.rs` (`CURRENT_VERSION` 6 -> 7):
  - `playlists(id INTEGER PRIMARY KEY, name TEXT NOT NULL, created_at, updated_at)`
  - `playlist_tracks(id INTEGER PRIMARY KEY, playlist_id REFERENCES playlists ON DELETE CASCADE, track_id REFERENCES tracks ON DELETE CASCADE, position INTEGER NOT NULL)`. The same track may appear twice, so it has its own row id.
- `store/playlists.rs` (with a `tests.rs`): create, rename, delete, list with counts, load tracks in order, append (many), remove entries, move entries (renumber positions in one transaction).
- Tests: CRUD, cascade on track removal, reorder, and a v6 -> v7 migration test that preserves starred data (modelled on `migrations.rs:35-105`).

### 2. m3u: new module in `crates/library` (or `crates/core`, no unsafe)
- `export_m3u(name, entries) -> String`: `#EXTM3U`, `#EXTINF:<secs>,<artist> - <title>`, then the path. Paths are relative to the file's directory when they sit beneath it, otherwise absolute. UTF-8 with `.m3u8` as the default extension.
- Remote tracks have no local path and are skipped, with a count reported to the user.
- Pure function with unit tests: escaping, relative paths, missing duration, unicode.
- Import (open `.m3u`) is a follow-up, not in scope here.

### 3. UI seam: `crates/ui`
- `library_api.rs`: `PlaylistInfo { id: u64, name, track_count }`, plus `LibraryDataSource` methods with default no-op impls: `playlists()`, `playlist_tracks(id)`, `create_playlist`, `rename_playlist`, `delete_playlist`, `add_to_playlist(id, &[u64])`, `remove_from_playlist(id, &[entry_ids])`, `move_in_playlist(id, entries, to)`.
- `PlaylistTrack` carries the entry id as well as the track id, because duplicates are allowed and selection is keyed by track id today.
- Real impl in `backend/library/mod.rs`, using the same lock, write, update snapshot, `mark_changed()` pattern as `set_starred`. `MockLibrary` gets an in-memory version (this is what the shell tests and `--mock` use).
- `Command` variants (`state/command.rs`): `CreatePlaylist`, `RenamePlaylist`, `DeletePlaylist`, `AddToPlaylist`, `RemoveFromPlaylist`, `MoveInPlaylist`, `ExportPlaylist { id, path }`, `PlayPlaylist { id, shuffle }`. Handled in `shell/commands.rs::apply_library_commands`, with tests on `MockLibrary`.
- **Playing needs no player change:** `PlayPlaylist { shuffle: false }` maps to `PlayTrack { id, context: ids }` and `shuffle: true` maps to `ShuffleScope { ids, label: name }`. Ids resolve to paths in `shell/commands.rs`, as for albums today.
- Export flow: the view asks for a destination with a thread-offloaded save-file helper (`save_picker.rs`, modelled on `folder_picker.rs`, using `rfd::FileDialog::save_file()`), then emits `ExportPlaylist`.

### 4. Navigator model: `panels/navigator.rs`, `state/view.rs`
`SECTIONS` is `&'static` and `View` is a `Copy` enum persisted in `Config.last_view`, so the plan is:
- Add `View::Playlist` (one variant, keeps `View: Copy`) and `AppState.selected_playlist: Option<PlaylistId>`. Persist the id in the UI state next to `last_view`. If the playlist is gone at startup, fall back to Music.
- The navigator owns a runtime `Vec<PlaylistInfo>` rendered as a **Playlists** section after `SECTIONS`. New messages: `AddPlaylist`, `SelectPlaylist(id)`, `RenamePlaylist`, `DeletePlaylist`, `DropTracks { playlist, ids }`.
- Update `every_view_appears_exactly_once` (`navigator.rs:91-130`) and `View::ALL`/`slug`, since the screenshot tools take a `--view` slug.
- `views/playlist.rs`: a `PlaylistView` modelled on `StarredView`, with a header (name, count, Play / Shuffle / Export buttons), `TrackTable` in **manual order** (no sort, since reordering needs a stable order), and a `ContextAction::RemoveFromPlaylist`.
- `TrackTable` context menus today act on a single row. Extend the message to carry the whole selection (`selection.selected_ids()` exists), so "Remove from playlist" and "Add to playlist >" work on multi-select.
- The track-table context menu gains a dynamic **Add to playlist >** submenu. This is the keyboard/accessible alternative to drag.

### 5. Frontends
Both must be done (docs/frontends.md), each with screenshots in both themes.

**frontend-win32** (`views/navigator.rs`, `views/track_table.rs`, `menu.rs`, `app.rs`)
- Extend the custom `NavigatorWidget`: runtime playlist rows, a hand-drawn `+` on the header (paint, `hit` and a new `NavigatorEvent::AddPlaylist`), a per-playlist context menu (Play, Shuffle, Rename, Export, Delete), a drop-hover highlight, UIA nodes (`nav-playlist-<id>`, and an invokable `+`), and F2/Delete keys.
- Name prompt for create/rename: a small modal window like `dialogs/tag_editor.rs`.
- `PlaylistView` reuses `TrackView` with a drag source and a reorder indicator.

**frontend-portable** (`views/navigator.rs`, `app/update.rs`, `app/layout.rs`, `menu.rs`)
- The xui `TreeView` rows have neither a header button nor ids: extend `build_rows` with a parallel id map, and add a header action. Add the layout/update matches for `View::Playlist`.
- xui has no drag and drop either (only `set_drag_region`); see gaps below.

## Phasing (one PR each, rebase-merge)
1. Storage + m3u export function + migration and tests.
2. `emusic-ui` seam: trait, commands, mock, shell tests, `PlaylistView`, navigator model. No frontend code, so it is testable headless.
3. win32ui prerequisites (see below), landed in `va1erian/win32ui`, then `scripts/bump-win32ui.sh`.
4. frontend-win32: navigator section, create/rename/delete, context menus, "Add to playlist" submenu, play/shuffle, export. Drag and drop lands last.
5. frontend-portable equivalent, after the xui gaps are closed or worked around.

Steps 1 and 2 are independent of win32ui, so the feature is fully usable through menus (add via submenu, remove via Delete, export, play) before drag and drop exists.

## win32ui gaps

win32ui is pinned at `265d41a` (`Cargo.lock`). The list below comes from grepping its `src` and README for the relevant APIs (DoDragDrop, IDropTarget, RegisterDragDrop, DragAcceptFiles, WM_DROPFILES, LVN_BEGINDRAG, TVN_BEGINDRAG, InsertMark, IFileSaveDialog, EditLabel, InputBox). None of these are present, and its docs don't list them as planned.

### Missing: needs win32ui changes
| # | Gap | Needed by | Proposed win32ui work |
|---|-----|-----------|-----------------------|
| 1 | **No drag source.** `ListViewEvent` (`controls/listview/events.rs:16-64`) has no begin-drag; `notify.rs` decodes no `LVN_BEGINDRAG`. Nothing exposes `DoDragDrop`, `IDropSource` or `IDataObject`. | Drag tracks to a playlist | `ListViewEvent::BeginDrag { rows }` (from `LVN_BEGINDRAG`/`LVN_BEGINRDRAG`), plus a drag-source helper that runs `DoDragDrop` with an in-app payload format and drop effects. |
| 2 | **No drop target.** `Custom` widgets get `MouseDown/Move/Up/Leave` and capture, but no drag events. | Navigator drop highlight, drop onto a playlist | `IDropTarget` on `Custom` widgets, surfaced as `Input::DragEnter/Over/Leave/Drop` with client coordinates. The same path gives file drop from Explorer (`.m3u` import, add files). |
| 3 | **No drag image, drop-effect cursor or auto-scroll.** | Feedback while dragging, long playlists | Use the OLE drag helper (`IDragSourceHelper`); auto-scroll driven from `DragOver`, using the existing `ListView::ensure_visible` and `Custom::scroll_into_view`. |
| 4 | **No list insertion marker or drag-reorder.** No `LVM_SETINSERTMARK`; `RowStyle` has no insert-line hook; `row_painter` has no cursor position. | Reorder inside a playlist | Add `ListView::set_insert_mark(Option<(row, after)>)` (or a painter hook), plus in-list drag using gaps 1-3. Model changes already work via `rows_changed`/`set_model`. |
| 5 | **No inline label editing** (`TVM_EDITLABEL`/`LVM_EDITLABEL`), and `Edit` exposes no key events, so no Escape-to-cancel. | Rename in the navigator | Preferred: a win32ui `Custom` in-place editor helper (overlay `Edit` with Enter/Escape/blur handling). Fallback: modal name prompt (see next row), which is what phase 4 ships. |
| 6 | **No input-box/prompt dialog helper.** | Create/rename playlist | Small `InputBox` dialog. The fallback is a frontend-win32 modal like `dialogs/tag_editor.rs`, so this is not blocking. |
| 7 | **No keyboard focus/navigation model in the navigator custom widget** (that widget is in this repo, but it relies on `Input::KeyDown` and `cx.focus()`, which exist). No `Key::APPS` constant. | F2/Delete/arrows in the navigator, Apps key menu | Add `Key::APPS` (`Key::from_code(0x5D)` works today). |
| 8 | **Keyboard context menu on `ListView` unverified.** `Message::ContextMenu { position: None }` exists (`sys/message/input.rs:129`) but `on_context` comes from `NM_RCLICK` (mouse only). | Shift+F10 / Apps on a selected track | Route `ContextMenu` with `position: None` to `on_context` for the focused row. Test first. |
| 9 | **No menu item icons.** | Nice-to-have in the playlist menu | Optional. |

### Partial: doable in frontend-win32 without win32ui changes
- **`+` on the section header:** hand-draw it in `NavigatorWidget::paint`, hit-test in `hit`, emit a new event. `ToolbarIcon::glyph('\u{E710}')` or the Segoe Fluent Icons font gives the glyph.
- **Dynamic navigator rows:** replace the static `SECTIONS` iteration in `for_each_row` with a runtime list, and call `set_content_height`. (win32ui's own `TreeView` supports dynamic children, but no drag, edit or header button, so the custom widget stays.)
- **Per-item context menu:** `Menu` and `Ui::popup` support submenus and separators, but `Msg::NavigatorContext` is keyed by `View`. Change it to a navigator item id.
- **"Add to playlist" submenu:** build the `Menu` on demand at right-click (menus are immutable after build).
- **Save dialog:** win32ui has none, but `rfd` (already used on a worker thread in `views/settings/playback/*`) provides `save_file()`. To verify: default extension for `.m3u`/`.m3u8`, and parenting to the win32ui window.
- **Accessibility:** add `Node` children with stable ids for the playlist rows and the `+`. `docs/win32-uia.md` already lists list scrolling and tree view patterns as gaps. Drag and drop has no UIA equivalent, so the menus, F2 and Delete are the accessible path.

### Exists (no work)
Popup menus with submenus/separators/disabled items; multi-select virtual `ListView` with `on_key` (Delete precedent: `views/now_playing/queue.rs:85`); `Edit` with `on_submit`; capture, cursor shape and scrolling on `Custom`; UIA hooks on custom widgets; secondary modal windows.

### xui (portable frontend), brief
xui-core has treeview, listview, edit, menu, popup and dialog widgets, but no drag and drop (only `set_drag_region` for window dragging) and no file dialogs. It needs the equivalent of gaps 1-4 (in-canvas drag is easier there, since the canvas backend owns all painting and input, so no OLE is needed for in-app drags) and `rfd` for dialogs. Needs its own assessment before phase 5.

## Open questions
- Should the same track be allowed twice in a playlist? The plan says yes (m3u and other players allow it), at the cost of entry ids. Say if you'd rather dedupe.
- Should drag-reorder be manual-only, or should a sortable column view also be offered (reorder disabled while sorted)? The plan is manual-only.
- Playlists containing remote-server tracks: skipped on m3u export. Is that acceptable?
- Should shuffle have its own persistent mode on the playlist header, or just the two Play / Shuffle buttons? The plan uses the buttons and reuses the global shuffle toggle.
