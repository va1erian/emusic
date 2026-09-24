# Performance baseline — egui build (issue #92)

Measured 2026-09-24 on `origin/main` at `e459863`
("Live-switch MIDI soundfonts via BASS_MIDI_StreamSetFonts").
`rustc 1.98.0`, `[profile.release]` (`lto = "thin"`, `codegen-units = 1`).

> The Win32 frontend (epic #91) and the `release-small` profile (#119)
> are compared against these numbers. Re-run with the commands below.

## Results

| Scenario | Startup to first window | Working set | Private bytes |
|---|---|---|---|
| Binary size (`emusic.exe`) | — | — | **16,262,144 bytes (15.5 MiB)** |
| Idle: Music view, empty library, nothing playing | 0.66 s | 237.8 MiB | 247.4 MiB |
| Playing: `--mock` (2,000 tracks), demo track playing | 0.46 s | 243.7 MiB | 263.8 MiB |
| Large mock library (50,000 tracks, scratch build): Albums view before scroll | 0.55 s | 403.5 MiB | 435.5 MiB |
| Same, after scrolling end-to-end (mock tiles are placeholders — see notes) | — | 404.8 MiB | 437.5 MiB |
| Real library (6,209 tracks / 615 albums): Albums view before scroll | 0.64 s | 268.0 MiB | 291.2 MiB |
| Same, after scrolling end-to-end (thumbnail cache full) | — | 272.1 MiB | 329.8 MiB |
| Same, +20 s later (plateau) | — | 272.2 MiB | 330.5 MiB |

Single sample per cell, taken 30 s after process start (plateau row: +50 s).
Working set = `Get-Process .WorkingSet64`, private bytes = `.PrivateMemorySize64`.

## How to re-run

Build once:

```powershell
cargo build --release -p emusic
(Get-Item target/release/emusic.exe).Length
```

Measure one scenario (startup = process start until `MainWindowHandle`
appears; memory sampled once, 30 s after start):

```powershell
$exe = "<repo>\target\release\emusic.exe"   # + "--mock" for the playing row
$p = Start-Process -FilePath $exe -PassThru # (-ArgumentList "--mock")
$start = Get-Date
while ($p.MainWindowHandle -eq 0) { $p.Refresh(); Start-Sleep -Milliseconds 100 }
"startup: $(((Get-Date) - $start).TotalSeconds) s"
Start-Sleep -Seconds (30 - ((Get-Date) - $start).TotalSeconds)
$m = Get-Process -Id $p.Id
"WS: $($m.WorkingSet64)  private: $($m.PrivateMemorySize64)"
Stop-Process -Id $p.Id -Force
```

Idle row: back up `%APPDATA%\emusic` and `%LOCALAPPDATA%\emusic` first, then
run with no folders configured (fresh config: Music view, visualizer off,
nothing playing). Restore the backups afterwards.

Playing row: `emusic.exe --mock` starts a demo track in `Playing` state
(`MockPlayer::playing_demo`), so the seek bar/time readouts repaint.
`--mock` ignores the real config (#135), so no backup is needed.

Albums scrolling: `--mock` always opens on Music (it starts from
`Config::default` with persistence disabled); preseed a real-mode run with
`last_view = "albums"` in `%APPDATA%\emusic\config.toml` instead. Keyboard
focus lands in the inner track table, so `PgDn`/`End` never reach the album
grid — click the grid, then send mouse-wheel events at its position (verified
via screenshots). The thumbnail LRU holds 300 textures max
(`views/album_grid/thumbs.rs`: `MAX_TEXTURES`, 4 uploads/frame), so memory
levels off once it is full — sample twice to confirm the plateau.

## Notes

- The issue guessed the seek bar repaints at ~30 fps while playing; the code
  actually does `request_repaint_after(1 s)` while playing without the
  visualizer (`app/update.rs`), and `visualizer::FRAME_INTERVAL` with it on.
- Mock mode generates 2,000 tracks / 200 albums (`mock/data.rs`). The 50,000
  row used a temporary `0..50_000u64` patch plus a temporary default-view
  patch (both reverted; release binary size was unaffected). Mock tiles have
  no artwork files, so their thumbnails always fail and the GPU texture cache
  stays empty in mock mode — the +48 MiB private delta only appears with a
  real library.
- Environment had no BASS DLLs, so real-mode runs used the inert
  `UnavailablePlayer` ("Audio unavailable" notice) — correct for idle, and
  the playing state is covered by `--mock` instead.
- No code changes in this commit; this file is the whole deliverable.
