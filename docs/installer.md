# Building the Windows installer

The installer is an [Inno Setup 7](https://jrsoftware.org/isdl.php) script
(`installer/emusic.iss`) that performs a **per-user** install of the 64-bit
build into `%LOCALAPPDATA%\Programs\emusic` — no admin rights, nothing
machine-wide. It copies `emusic.exe`, an `icons/` folder (the committed icon
set, #125) and, when present, a `bass/` folder, adds a Start Menu shortcut,
registers the file associations after install and removes them again on
uninstall.

## Prerequisites

- A stable Rust toolchain (see `rust-toolchain.toml`).
- Inno Setup 7, ideally the **64-bit edition** (`winget install JRSoftware.InnoSetup.7`).
  Both editions can build 64-bit installers; the 64-bit one compresses faster.
- The BASS x64 DLLs, downloaded separately from <https://www.un4seen.com>
  (see the [BASS DLLs](../README.md#bass-dlls) section of the README). They
  are **not** in git.

## 1. Build the executable

From the repository root:

```
cargo build --release
```

This produces `target\release\emusic.exe`.

## 2. Place the BASS DLLs

Put the x64 BASS DLLs in `target\release\bass\` (the same `bass\` folder the
app loads at runtime from `<exe dir>\bass\`). At minimum `bass.dll`; the codec
DLLs (`bassflac.dll`, `bassopus.dll`, …) are optional.

The DLLs are only needed at **build time** for the installer to pick them up.
If `bass\` is missing the .iss still compiles, but with a compiler warning and
the resulting installer will ship an emusic that cannot play audio.

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
  `bass\*.dll` (skipped when absent).
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
- Everything else (`DirExists`/`FileExists`/`AddBackslash`/`#pragma warning`)
  is long-standing ISPP and kept deliberately conservative.

A GitHub Actions workflow that builds and attaches the installer on a tag is
deliberately out of scope for #29; the manual packaging step above is the
supported path for now. Adding the workflow and a documented
"download BASS + build installer" release job is a follow-up.
