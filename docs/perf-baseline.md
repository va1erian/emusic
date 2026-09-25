# Performance baseline — binary size and memory (#92, #239, #119)

The egui baseline (#92) was measured 2026-09-24 on `origin/main` at `e459863`
("Live-switch MIDI soundfonts via BASS_MIDI_StreamSetFonts").

The Win32 and `release-small` comparisons (#239, #119) were measured
2026-09-25 with `rustc 1.98.0` on `feat/119-release-small` atop `a03e078`
("Win32: add a clear button to the top-bar search box (#244)"), the same
toolchain as the egui baseline. Memory was sampled on the same branch before
that one-commit rebase; the extra top-bar button does not change it.

Both frontends use `[profile.release]` (`lto = "thin"`, `codegen-units = 1`);
`release-small` (#119) adds `opt-level = "z"`, `lto = "fat"`,
`codegen-units = 1`, `panic = "abort"` and `strip = true`.

## Binary size

`target/<profile>/emusic.exe` (egui) and `emusic-win32.exe` (Win32):

| Build | release | release-small |
|---|---|---|
| egui (`emusic.exe`) | **17,981,440 B (17.15 MiB)** | **10,255,872 B (9.78 MiB)** |
| Win32 (`emusic-win32.exe`, WIC) | **12,024,320 B (11.47 MiB)** | **6,077,952 B (5.80 MiB)** |
| Δ Win32 vs egui | −5,957,120 B (−33.1%) | −4,177,920 B (−40.7%) |

`release-small` shrinks the egui binary by 7,725,568 B (−43.0%) and the Win32
binary by 5,946,368 B (−49.5%).

### WIC vs the `image` crate (Win32, #119)

The optional WIC decode was checked by building the same commit with the
`image`-based decoder (pre-change) and with `win32ui::imaging` (WIC):

| Win32 build | `image` crate | WIC | Δ |
|---|---|---|---|
| release | 12,570,112 B (11.99 MiB) | 12,024,320 B (11.47 MiB) | **−545,792 B (−4.3%)** |
| release-small | 6,391,808 B (6.09 MiB) | 6,077,952 B (5.80 MiB) | **−313,856 B (−4.9%)** |

WIC measurably reduces the Win32 binary, so the change is kept. The `image`
crate is no longer referenced from the Win32 binary at all: the string
`image-0.25` appears 7 times in `emusic.exe` but 0 times in the WIC
`emusic-win32.exe` (checked with `rg -a -c`). WIC decoding uses the system
JPEG/PNG codecs, scaling uses WIC's resampler, and the on-disk thumbnail cache
is (re)written as JPEG by WIC at the same `%LOCALAPPDATA%\emusic\thumbs\
<path-hash>.jpg` names the egui frontend uses, so both share one cache.
`emusic-ui` still depends on `image` for the egui frontend; only the Win32
binary drops it.

## Startup and memory

Single sample per cell, taken 30 s after process start (matching #92).
Working set = `Get-Process .WorkingSet64`; private bytes =
`.PrivateMemorySize64`, both in MiB. Real-mode runs had no BASS DLLs next to
the executable, so they used the inert `UnavailablePlayer`; playback memory is
covered by `--mock`.

### egui baseline (#92, for reference)

| Scenario | Startup to first window | Working set | Private bytes |
|---|---|---|---|
| Binary size (`emusic.exe`) | — | — | **17,981,440 B (17.15 MiB)** |
| Idle: Music view, empty library, nothing playing | 0.66 s | 237.8 | 247.4 |
| Playing: `--mock` (2,000 tracks), demo track playing | 0.46 s | 243.7 | 263.8 |
| Large mock library (50,000 tracks, scratch build): Albums before scroll | 0.55 s | 403.5 | 435.5 |
| Same, after scrolling end-to-end (mock tiles are placeholders) | — | 404.8 | 437.5 |
| Real library (6,209 tracks / 615 albums): Albums before scroll | 0.64 s | 268.0 | 291.2 |
| Same, after scrolling end-to-end (thumbnail cache full) | — | 272.1 | 329.8 |
| Same, +20 s later (plateau) | — | 272.2 | 330.5 |

### Win32 frontend vs egui (#239)

`[profile.release]`, 30 s after start:

| Scenario | egui release WS / PB | Win32 release WS / PB | Δ WS |
|---|---|---|---|
| Idle: Music view, empty library | 227.0 / 245.0 | 54.6 / 68.1 | −172.4 |
| `--mock` (2,000 tracks), demo track | 242.6 / 259.8 | 66.1 / 81.1 | −176.5 |
| Real library: Music view | 272.8 / 289.4 | 73.6 / 83.5 | −199.2 |
| Real library: Albums before scroll | 276.1 / 303.8 | 66.2 / 82.4 | −209.9 |
| Real library: Folders view | 283.0 / 301.2 | 66.1 / 83.9 | −216.9 |

The Win32 frontend holds roughly a quarter of the egui frontend's working set.
That is expected: it draws with GDI/common controls instead of loading the
OpenGL/egui stack and fonts, and `--mock` artwork is placeholders only.

### `release-small` (#119)

Same scenarios and method, 30 s after start:

| Scenario | egui WS / PB | Win32 WS / PB |
|---|---|---|
| Idle: Music view, empty library | 225.8 / 245.5 | 50.6 / 67.1 |
| `--mock` (2,000 tracks), demo track | 241.0 / 257.5 | 65.3 / 80.9 |
| Real library: Music view | 271.6 / 291.6 | 65.5 / 84.1 |
| Real library: Albums before scroll | 276.7 / 308.7 | 65.5 / 84.9 |
| Real library: Folders view | 276.4 / 296.1 | 65.5 / 83.8 |

Memory is essentially unchanged between `release` and `release-small` (within
sample noise): the profile trades code size for speed, not run-time memory.

Startup to a top-level window handle was ~0.07 s (egui) and ~0.20 s (Win32) in
this run. egui creates its window before the first frame is painted, so its
figure under-reports vs #92's 0.4–0.7 s (which waited for the window to
appear); the Win32 number is the more honest of the two here. Startup is not
the interesting axis for either build.

### Observations

- The dominant size contributors remain the `windows`/WinRT and egui/DirectX
  bindings and `image` (egui): the Win32 binary still carries the `windows`
  crate (GDI, Direct2D, Dwm, WIC); the egui binary additionally carries
  `wgpu`/`glow`/`epaint`/fonts and `image`. BASS is a small DLL loaded at
  run time, not linked into either binary.
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
cargo build --release -p emusic -p emusic-win32
cargo build --profile release-small -p emusic -p emusic-win32
(Get-Item target/release/emusic.exe).Length
(Get-Item target/release/emusic-win32.exe).Length
```

Measure one scenario (startup = process start until `MainWindowHandle`
appears; memory sampled once, 30 s after start):

```powershell
$exe = "<repo>\target\release\emusic-win32.exe"   # or emusic.exe; + "--mock"
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
`--mock` ignores the real config (#135), so no backup is needed. Both
frontends accept it.

## Notes

- `--mock` data is 2,000 tracks / 200 albums (`ui/src/mock/data.rs`); mock
  tiles have no artwork files, so their thumbnails always fail and the texture
  cache stays empty. The real-library artwork delta only appears with a real
  library and a scroll (see #92's plateau row).
- The Win32 frontend has no visualizer and no Milkdrop, so real-mode idle is
  lower than egui even with the visualizer disabled.
- The Win32 run does not yet have an Albums-after-scroll measurement: the
  committed #92 egui plateau row (272.2 MiB WS / 330.5 MiB PB) is the reference
  until Win32 input automation is added. Win32 does now have a Folders row.
- Not all egui views exist in Win32 yet (epic #91); treat the Win32 numbers as
  a floor.
- The WIC integration adds `win32ui::imaging` (github.com/va1erian/win32ui,
  `main` since the `feat/wic-imaging` work merged); the Win32 manifest tracks
  `main`.
