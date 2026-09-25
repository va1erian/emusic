# Running UI tests without taking over your desktop

emusic's test suite opens real top-level windows, moves focus
(`SetForegroundWindow`), sends synthetic input and renders Direct2D frames. On
a shared desktop that steals focus while you (or another agent) type, and your
typing can make the tests flaky.

`scripts/sandbox/run.ps1` runs them inside **Windows Sandbox**, a throwaway
Hyper-V VM that ships with Windows. It has its own desktop, input queue and
foreground window, so nothing inside reaches the host. The script is a copy of
the one in [va1erian/win32ui](https://github.com/va1erian/win32ui)
(`scripts/sandbox/run.ps1`), adapted to emusic's workspace: it stages the BASS
DLLs, and gives every test binary a unique name and the working directory it
expects.

## One-time setup (Windows 10/11 Pro, Enterprise or Education)

```powershell
# as administrator, then reboot
Enable-WindowsOptionalFeature -Online -FeatureName Containers-DisposableClientVM
```

You install nothing else. The sandbox doesn't need Rust or Visual Studio. The
**host** still needs the normal build prerequisites (the toolchain, and the
MinGW-w64 GCC that `crates/sid` compiles cRSID with — see `AGENTS.md`).

## Usage

```powershell
# Every test in the workspace
scripts\sandbox\run.ps1

# The CI feature set
scripts\sandbox\run.ps1 -CargoArgs '--features','emusic/shot'

# One integration test binary, with a libtest filter
scripts\sandbox\run.ps1 -CargoArgs '--test','smoke' -TestArgs 'music'

# Run the tests marked #[ignore] too
scripts\sandbox\run.ps1 -TestArgs '--include-ignored'

# The real BASS-backed tests (otherwise they skip for lack of bass*.dll)
scripts\sandbox\run.ps1 -BassDir C:\BASS\x64
```

How it works:

1. Builds on the **host** with `cargo test --no-run` (or `cargo build` with
   `-Build`), into `target\sandbox`, with a static CRT — the sandbox image has
   no Visual C++ runtime.
2. Stages everything under `target\sandbox\sandbox-stage`:
   - `bin\` — the test/executable binaries, each renamed uniquely (two crates
     both have a `smoke.exe`);
   - `pkg\<crate>\` — the crate's `tests\` folder and other runtime data, so a
     binary can run from the crate root it expects;
   - `bass\` — `bass*.dll` copied from `-BassDir`/`EMUSIC_BASS_DIR`;
   - `out\` — the logs and screenshots that come back to you.
3. Starts a network-less sandbox (no vGPU by default), maps that folder as
   `C:\stage` and runs every binary there, writing `out\<name>.log` and
   `out\summary.txt`. The host script then closes the sandbox itself.
4. Prints the logs and exits with code 1 if any binary failed.

A cold start takes about 15–30 s on top of the build. You can leave the sandbox
window behind your other windows, because nothing inside it touches the host's
focus or input. Use `-Keep` to leave it open afterwards so you can look around.

## Screenshots and visual reproduction

The sandbox has its own desktop with nothing else on it, so a plain full-screen
capture is enough, and focus and occlusion don't matter:

```powershell
# The real app against deterministic mock data: launch, capture, stop
scripts\sandbox\run.ps1 -Build -CargoArgs '--bin','emusic' `
    -TestArgs '--mock' -Screenshot
```

The PNG lands in `target\sandbox\sandbox-stage\out\<name>.png`; `-Screenshot`
stops the app after the capture, so no autoclose switch is needed.

The deterministic shot tools work too — point their `--out` at `C:\stage\out`
so the PNGs come back to the host:

```powershell
# One PNG per view, in both themes, with DWM frame and Mica
scripts\sandbox\run.ps1 -Build `
    -CargoArgs '--features','emusic/shot','--bin','emusic-shot' `
    -TestArgs '--all','--theme','light','--out','C:\stage\out\light'
```

`-Exe <path>` runs an executable you already built, skipping cargo entirely.

## What the script fixes over a plain `cargo test`

- **BASS.** `cargo test` on a machine without BASS silently skips the
  BASS-backed tests. Pass `-BassDir` and they run for real: the DLLs are staged
  and `EMUSIC_BASS_DIR` is set inside the sandbox.
- **Working directory.** Some tests read files relative to their crate root.
  The script stages `tests\` and runs each binary from its crate root, so those
  tests really run.
- **Name collisions.** `emusic` and `win32ui-demo` both build `smoke.exe`; a
  single `bin\` folder would overwrite one. Each gets a unique name.
- **Runtime.** A static CRT makes the binaries self-contained (no
  `vcruntime140.dll` in the sandbox image).

## Limitations and gotchas

- **One sandbox at a time.** If the script says one is already running, it
  probably belongs to someone else: wait for it or report it. Never kill it.
- **vGPU.** The sandbox's virtual GPU is **off by default**. On some GPU
  drivers it makes the whole VM die (`0x80370106`) as soon as a GPU test
  starts. WARP (the software rasteriser) renders the app's Direct2D/Mica
  correctly, so the default is the reliable one; pass `-EnableVgpu` only if you
  need a hardware GPU path.
  The script detects a mid-run VM death early and tells you so instead of
  waiting out the timeout.
- **Skips are silent.** Tests that need BASS skip with a message in
  the log rather than failing. Read `out\<name>.log` before believing a green
  run; `-BassDir` removes the BASS skips.
- **Doctests don't run.** `cargo test --no-run` doesn't produce doctest
  executables. Run `cargo test --doc` on the host — it doesn't touch the
  desktop. Tests marked `#[ignore]` also don't run unless you pass
  `-TestArgs '--include-ignored'`.
- **UI Automation can't reach into the sandbox.** The sandbox is a separate
  VM, so `scripts\win32-uia.ps1` on the host can't drive an app running inside
  it. Use `-Screenshot` for visuals here; run UIA on the host or in CI when you
  need real clicks and keystrokes (see [win32-uia.md](win32-uia.md)).
- **Disk.** The static-CRT build in `target\sandbox` is a second, cold build
  (several GB). Use `-TargetDir` to point it at a shared location, and delete
  it when you're done with a branch.
- **First build needs network.** Cargo fetches the `win32ui` git dependency on
  the host the first time. The sandbox itself is network-less.
- **CI.** The `windows-latest` GitHub runner is already a disposable VM with an
  interactive desktop, so the `ci.yml` jobs are sandboxed by design. Windows
  Sandbox can't run on hosted runners (no nested virtualization), so
  `run.ps1` is for local use.
