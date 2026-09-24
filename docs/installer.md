# Building the Windows installers

The installer is an [Inno Setup 7](https://jrsoftware.org/isdl.php) script
(`installer/emusic.iss`) that performs a **per-user** install of the 64-bit
egui build into `%LOCALAPPDATA%\Programs\emusic` — no admin rights, nothing
machine-wide. It copies `emusic.exe`, an `icons/` folder (the committed icon
set, #125) and a **required** `bass/` folder (the x64 BASS DLLs, #124), adds a
Start Menu shortcut, registers the file associations after install and removes
them again on uninstall.

A companion script, `installer/emusic-win32.iss`, packages the native Win32
frontend (`emusic-win32.exe`, #91) the same way, as a separate installer that
can be installed side by side with the egui build — see
[The Win32 installer](#the-win32-installer) below.

## Prerequisites

- A stable Rust toolchain (see `rust-toolchain.toml`).
- Inno Setup 7, ideally the **64-bit edition** (`winget install JRSoftware.InnoSetup.7`).
  Both editions can build 64-bit installers; the 64-bit one compresses faster.
- The BASS x64 DLLs, downloaded separately from <https://www.un4seen.com>
  (see the [BASS DLLs](../README.md#bass-dlls) section of the README). They
  are **not** in git, and they are a **required** packaging input: the script
  fails to compile without them (#124).

## 1. Build the executable

From the repository root:

```
cargo build --release
```

This produces `target\release\emusic.exe`.

## 2. Place the BASS DLLs

Put the x64 BASS DLLs in `target\release\bass\` (the same `bass\` folder the
app loads at runtime from `<exe dir>\bass\`). This is a **required** step
(#124): if the folder is missing or contains no `bass*.dll`, the compile fails
with an `#error` instead of producing a BASS-less installer.

`bass.dll` is the runtime core the app loads on startup; the codec add-ons are
loaded automatically, one per file, for every other `bass*.dll` found in the
folder (see `crates/bass/src/ffi/loader.rs` and
`crates/app/src/backend/mod.rs`). The maintainer's packaging set is:

- `bass.dll` (required — audio engine)
- `bassflac.dll` (FLAC)
- `bassopus.dll` (Opus)
- `bassmidi.dll` (MIDI)

Drop in any other BASS add-on for the formats you want covered (`bassalac.dll`,
`basswv.dll`, `bassape.dll`, `bass_mpc.dll`, `basswebm.dll`, …); whatever is in
the folder is copied and auto-loaded. The full download bundle is listed in the
[BASS DLLs](../README.md#bass-dlls) section of the README.

The DLLs are only read at **build time** so the installer can pick them up.
The installer additionally writes `{app}\bass\README.txt` at install time with
a short attribution/license notice for BASS (see below).

## 3. Compile the installer

Run Inno Setup 7's command-line compiler (`ISCC.exe`), or open
`installer\emusic.iss` in the Inno Setup 7 IDE and press F9:

```
ISCC.exe installer\emusic.iss
```

The output is `installer\emusic-<version>-setup.exe`.

To build against DLLs kept somewhere else (e.g. the unzipped BASS release),
pass the locations as defines instead of copying files:

```
ISCC.exe /DBuildDir=C:\path\to\release /DBassDir=C:\path\to\bass\  installer\emusic.iss
```

`BuildDir` defaults to `<repo>\target\release`; `BassDir` defaults to
`<BuildDir>\bass`.

## What the installer does

- Installs to `{localappdata}\Programs\emusic` (`PrivilegesRequired=lowest`,
  so no UAC prompt).
- Copies `emusic.exe`, `icons\*.ico` (the committed icon set, #125) and
  `bass\*.dll` (required, #124), and writes a `bass\README.txt` attribution
  notice next to them.
- Creates a Start Menu shortcut.
- Runs `emusic.exe --register-associations` after install (silently), and
  `emusic.exe --unregister` before uninstall, matching the flags parsed in
  `crates/app/src/cli.rs`.

## Icons

`assets/` holds the icon set (#125): `ico/` multi-resolution `.ico` files
(the app, setup and uninstall icons plus one `file-<ext>.ico` per supported
extension, with `file-audio.ico` as the generic fallback), `png/` renders at
16–256 px and `svg/` sources. Unlike the BASS DLLs it is committed to the
repository, so no download step is needed.

The installer embeds `assets\ico\emusic-setup.ico` as its own icon, copies
every `assets\ico\*.ico` into `{app}\icons\`, and points
`UninstallDisplayIcon` at `{app}\icons\emusic-uninstall.ico`. At registration
time `emusic.exe` resolves that `icons\` folder relative to itself and sets
each extension's `DefaultIcon` to its `file-<ext>.ico` (or `file-audio.ico`),
so Explorer shows the right icon per file type.

`AssetsDir` defaults to `<repo>\assets` and can be overridden with
`/DAssetsDir=<path>` if the icon set lives elsewhere.

## BASS attribution

BASS is free for non-commercial use, and emusic is personal MIT-licensed
freeware, so it is covered by that license. The DLLs are never committed and
are only pulled in at packaging time. As good practice (and because the BASS
license keeps its names the property of their owners), the installer writes a
short notice to `{app}\bass\README.txt` crediting BASS/un4seen and pointing at
the `bass.txt` license files from the BASS download. The uninstaller removes
that file again via `[UninstallDelete]`.

A commercial fork or build of emusic would need its own BASS license from
<https://www.un4seen.com> — emusic's MIT license does not change BASS's terms.

## Inno Setup 7 notes

- **`SetupArchitecture=x64`** (new in IS7) makes ISCC emit a true 64-bit
  installer instead of the default 32-bit one. That matches the x64 app/DLLs
  and enables high-entropy ASLR; IS7 already defaults `ArchitecturesAllowed`
  and `ArchitecturesInstallIn64BitMode` to `x64compatible`, which the script
  spells out for clarity. This directive is IS7-only, so the script requires
  Inno Setup 7 (the maintainer's target) and will not compile with IS 6.
- The new IS7 default `AppVerName` (`"<AppName> <AppVersion>"`) is relied upon
  rather than spelling out the pre-7 `"… version …"` form.
- `WizardStyle=modern dynamic` (IS 6.6+, still current in 7) follows the
  Windows light/dark setting.
- Everything else (`DirExists`/`FileExists`/`AddBackslash`/`FindFirst`/
  `#error`/`#pragma warning`) is long-standing ISPP and kept deliberately
  conservative. `FindFirst` is used only to prove the `bass\` folder has at
  least one `bass*.dll` (an empty folder must fail too, #124).

A `.github/workflows/release.yml` workflow builds both installers and a
portable zip for each on every `v*` tag and attaches all four assets to the
GitHub Release (#240), using the same BASS-download and packaging steps
described above.

## The Win32 installer

`installer/emusic-win32.iss` packages the native Win32 frontend
(`emusic-win32.exe`, #91) as a **separate** per-user install so it can sit
side by side with the egui build instead of replacing it:

- a different display name ("emusic (Win32)"), `AppId` GUID, install dir
  (`{localappdata}\Programs\emusic-win32`), Start Menu shortcut and uninstall
  entry — installing or uninstalling one build never touches the other;
- the same `bass\` and `icons\` packaging inputs and the same `BuildDir`/
  `BassDir`/`AssetsDir` overrides as `emusic.iss`, since `cargo build
  --release --workspace` produces both exes into one `target\release\`;
- `OutputBaseFilename=emusic-win32-{#AppVersion}-setup`, so
  `ISCC.exe installer\emusic-win32.iss` produces
  `installer\emusic-win32-<version>-setup.exe`.

Build it the same way as the egui installer:

```
ISCC.exe installer\emusic-win32.iss
```

### Shared data, one set of file associations

Both frontends read and write the same `%APPDATA%\emusic` config/library
directory (`emusic_ui::config`) — installing both is safe, they see the same
library. File associations are different: both frontends register under the
same `AssocManager::new("emusic")` name (`crates/app/src/main.rs` and
`crates/win32/src/main.rs`), so there is only **one** active default-player
registration at a time, whichever frontend last ran
`--register-associations`. The egui installer registers unconditionally on
install; the Win32 installer instead makes it an opt-in `[Tasks]` checkbox
("Make emusic (Win32) the default player...", unchecked by default) so
installing the Win32 build next to the egui build does not silently steal its
file associations. Both installers always run `--unregister` on uninstall
regardless of that task, which is a harmless no-op if nothing was
registered.
