# Driving the Win32 frontend with UI Automation

The native frontend (`crates/win32`) exposes its controls to Windows UI
Automation (UIA) through `win32ui`'s accessibility providers. Screen readers use
the same tree, and so can agents and tests: read state, click, type and select
without pixel coordinates.

## Quick start

```powershell
cargo build -p emusic-win32
. .\scripts\win32-uia.ps1
$app = Start-Emusic                 # `emusic-win32 --mock`, deterministic data
Show-UiaTree $app -MaxDepth 2
Invoke-Uia $app 'Settings'          # File > Settings (menu items invoke directly)
Invoke-Uia $app 'Playback'          # a tab
Click-Uia $app 'Smooth'             # real mouse click at the element's centre
Test-Responding $app
Stop-Emusic $app
```

`Invoke-Uia` calls the control's action directly and skips the message pump.
Use `Click-Uia` / `Send-Key` (real input) for anything that depends on mouse or
keyboard handling; that is how the Settings preset-button freeze was found (it
only reproduced with real input). Always `Stop-Emusic` when done.

## What the tree looks like

| Area | Elements |
| --- | --- |
| Menu bar | `MenuBar` > `MenuItem` (File, View) > submenu items. Invoke runs the item; no popup opens. |
| Top bar | `ToolBar 'Top bar'`: transport buttons (`Previous`, `Pause`, ...), `Repeat`/`Shuffle` check boxes, `Seek` and volume sliders (RangeValue), `Clear search`. Ids are `top-bar-<n>`. |
| Navigator | `List 'Navigator'` > `ListItem` per view (`nav-music`, `nav-albums`, ...); Select/Invoke switches view. |
| Lists | `List` > visible rows only (`ListItem`, name = the row's non-empty cells, id `row-<n>`) > one `Text` per cell (id = column title). Select selects; Invoke activates (double-click). Scroll to reach other rows. |
| Settings | Tabs (`Library`, `Appearance`, ...) as `TabItem`; buttons, check boxes, radios and sliders by their label. |
| Search | `Edit` named by its cue text; Value pattern reads and sets the text. |
| Status bar | `StatusBar` > `Text` per part. |

Give a control a stable handle with `ControlExt::set_accessible_name` /
`set_accessible_id` (`win32ui`). A custom widget describes itself by
implementing `CustomWidget::accessibility` and `accessibility_action`
(see `views/navigator.rs` for a small example).

## Gotchas

- UIA drops zero-size elements: a control must have real bounds to appear.
- `FindFirst(Descendants, ...)` over the track table is slow (thousands of
  rows); `Find-Uia` skips native lists unless `-IncludeLists`.
- The debug build is slow under UIA; expect seconds, not milliseconds.
- Known gaps (tracked in `win32ui`): grid view tiles, popup menu contents,
  list-view scrolling patterns, tree view, document text. See the follow-up
  issues to #26 in `va1erian/win32ui`.

## Diagnosing a hung UI

If `Test-Responding` is `$false`: `Save-Dump $app "$env:TEMP\hung.dmp"`, then read
the main thread's stack (the first thread in the dump; return addresses in
`emusic-win32.exe` symbolize with `llvm-symbolizer --relative-address
--obj=target/debug/emusic-win32.exe`). A busy loop shows CPU time still rising
in `Get-Process`; a deadlock does not.
