# Performance baseline — binary size and memory (#92, #239, #119)

The baseline was measured 2026-09-25 with `rustc 1.98.0` on
`feat/119-release-small` atop `a03e078` ("Win32: add a clear button to the
top-bar search box (#244)"). Memory was sampled on the same branch before that
one-commit rebase; the extra top-bar button does not change it.

The app uses `[profile.release]` (`lto = "thin"`, `codegen-units = 1`);
`release-small` (#119) adds `opt-level = "z"`, `lto = "fat"`,
`codegen-units = 1`, `panic = "abort"` and `strip = true`.

## Binary size

`target/<profile>/emusic.exe`:

| Build | release | release-small |
|---|---|---|
| `emusic.exe` (WIC) | **12,024,320 B (11.47 MiB)** | **6,077,952 B (5.80 MiB)** |

`release-small` shrinks the binary by 5,946,368 B (−49.5%).

### WIC vs the `image` crate (#119)

The optional WIC decode was checked by building the same commit with the
`image`-based decoder (pre-change) and with `win32ui::imaging` (WIC):

| Build | `image` crate | WIC | Δ |
|---|---|---|---|
| release | 12,570,112 B (11.99 MiB) | 12,024,320 B (11.47 MiB) | **−545,792 B (−4.3%)** |
| release-small | 6,391,808 B (6.09 MiB) | 6,077,952 B (5.80 MiB) | **−313,856 B (−4.9%)** |

WIC measurably reduces the binary, so the change is kept. The `image` crate is
no longer linked into the binary at all: the string `image-0.25` appears 0
times in `emusic.exe` (checked with `rg -a -c`). WIC decoding uses the system
JPEG/PNG codecs, scaling uses WIC's resampler, and the on-disk thumbnail cache
is (re)written as JPEG by WIC at `%LOCALAPPDATA%\emusic\thumbs\<path-hash>.jpg`.

## Startup and memory

Single sample per cell, taken 30 s after process start (matching #92).
Working set = `Get-Process .WorkingSet64`; private bytes =
`.PrivateMemorySize64`, both in MiB. Real-mode runs had no BASS DLLs next to
the executable, so they used the inert `UnavailablePlayer`; playback memory is
covered by `--mock`.

### `[profile.release]`, 30 s after start

| Scenario | Working set | Private bytes |
|---|---|---|
| Idle: Music view, empty library | 54.6 | 68.1 |
| `--mock` (2,000 tracks), demo track | 66.1 | 81.1 |
| Real library: Music view | 73.6 | 83.5 |
| Real library: Albums before scroll | 66.2 | 82.4 |
| Real library: Folders view | 66.1 | 83.9 |

### `release-small` (#119)

Same scenarios and method, 30 s after start:

| Scenario | Working set | Private bytes |
|---|---|---|
| Idle: Music view, empty library | 50.6 | 67.1 |
| `--mock` (2,000 tracks), demo track | 65.3 | 80.9 |
| Real library: Music view | 65.5 | 84.1 |
| Real library: Albums before scroll | 65.5 | 84.9 |
| Real library: Folders view | 65.5 | 83.8 |

Memory is essentially unchanged between `release` and `release-small` (within
sample noise): the profile trades code size for speed, not run-time memory.

Startup to a top-level window handle was ~0.20 s in this run.

### Observations

- The dominant size contributors remain the `windows`/WinRT bindings (GDI,
  Direct2D, Dwm, WIC). BASS is a small DLL loaded at run time, not linked into
  the binary.
- The `panic = "abort"` in `release-small` is safe here: no Rust code needs to
  recover from a panic. BASS sync callbacks run through an `extern "system"`
  trampoline (`crates/bass/src/sync.rs`), and Rust already aborts a panic that
  would cross that FFI boundary (there is no `catch_unwind` there), so `abort`
  changes nothing for them. The only behavioural difference is the library
  scanner (`crates/library/src/scanner/mod.rs`,
  `crates/ui/src/backend/library/scan.rs`), which re-raises a worker panic via
  `resume_unwind`; with `abort` that panic stops the process instead of being
  caught, which is acceptable for an explicitly opt-in size-first build.

## How to re-run

Build one configuration:

```powershell
cargo build --release -p emusic
cargo build --profile release-small -p emusic
(Get-Item target/release/emusic.exe).Length
```

Measure one scenario (startup = process start until `MainWindowHandle`
appears; memory sampled once, 30 s after start):

```powershell
$exe = "<repo>\target\release\emusic.exe"        # + "--mock" for the mock row
$p = Start-Process -FilePath $exe -PassThru       # (-ArgumentList "--mock")
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
nothing playing). Restore the backups afterwards. To select a view, set
`last_view = "music" / "albums" / "folders"` in
`%APPDATA%\emusic\config.toml`.

Playing row: `--mock` starts a demo track in `Playing` state
(`MockPlayer::playing_demo`), so the seek bar/time readouts repaint.
`--mock` ignores the real config (#135), so no backup is needed.

## Notes

- `--mock` data is 2,000 tracks / 200 albums (`ui/src/mock/data.rs`); mock
  tiles have no artwork files, so their thumbnails always fail and the texture
  cache stays empty. The real-library artwork delta only appears with a real
  library and a scroll.
- The app has no Milkdrop/projectM renderer yet (see #295), so real-mode idle
  is measured with the visualizer off.
- The Albums-after-scroll row still needs input automation to reproduce; the
  Folders row is measured.
- The WIC integration adds `win32ui::imaging` (github.com/va1erian/win32ui,
  `main` since the `feat/wic-imaging` work merged); the app's manifest tracks
  `main`.
