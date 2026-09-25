# Building the Windows installers

The installer is an [Inno Setup 7](https://jrsoftware.org/isdl.php) script
(`installer/emusic.iss`) that performs a **per-user** install of the 64-bit
app into `%LOCALAPPDATA%\Programs\emusic` — no admin rights, nothing
machine-wide. It copies `emusic.exe`, an `icons/` folder (the committed icon
set, #125) and a **required** `bass/` folder (the x64 BASS DLLs, #124), adds a
Start Menu shortcut, registers the file associations after install and removes
them again on uninstall.

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
`crates/ui/src/backend/mod.rs`). The maintainer's packaging set is:

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
  `crates/ui/src/cli.rs`.

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
- The preset download code (#299) uses `ExtractArchive`, which was added in
  Inno Setup **6.4**; it also sets `ArchiveExtraction=enhanced/nopassword`.
  The emusic installer already requires Inno Setup 7, so 7 stays the minimum
  for building it — 6.4 is only the floor for the shared `presets.iss`.
- Everything else (`DirExists`/`FileExists`/`AddBackslash`/`FindFirst`/
  `#error`/`#pragma warning`) is long-standing ISPP and kept deliberately
  conservative. `FindFirst` is used only to prove the `bass\` folder has at
  least one `bass*.dll` (an empty folder must fail too, #124).

A `.github/workflows/release.yml` workflow builds the installer and a portable
zip on every `v*` tag and attaches both assets to the GitHub Release (#240),
using the same BASS-download and packaging steps described above.

## Visualization presets

The full MilkDrop preset collection is roughly 140 MB, so the installer only
**bundles** the small base set and offers the rest as an optional download
(#299). Nothing is committed to the repository: packs are fetched from the
projectM preset repositories at pinned commits and published as `.7z` release
assets when needed.

- **Bundled base set.** `scripts\fetch-presets.ps1` downloads and stages
  `milkdrop-original` (552 presets) and the `textures` pack (67 textures, used
  by many presets) under `target\presets\`. `installer\emusic.iss` copies them
  to `{app}\visualizations\presets\milkdrop-original\` and
  `{app}\visualizations\textures\`. Missing presets are a compile **warning**,
  not an error, so a local build still works without the download; override
  the location with `/DPresetsDir=<path>`.
- **Optional downloads.** The Select Components page lists the large packs
  (`cream-of-the-crop`, `en-d` and `projectm-classic`, the last unchecked by
  default). `installer\presets.iss` builds a download page, downloads each
  selected archive, checks its SHA-256 and extracts it with `ExtractArchive`
  into `{app}\visualizations\presets\<pack>\`. Failures are non-fatal and
  reported, and the bundled base set keeps working.
- **Get more presets later.** `installer\emusic-presets.iss` builds
  `emusic-presets-setup.exe`, a tiny stand-alone installer that reads the
  install directory from the main installer's uninstall key
  (`AppId {B35420CE-…}`), shows the same pack checkboxes (pre-checked when a
  pack is already installed) and reuses `presets.iss`. The main installer
  bundles the helper into `{app}\`, so it is always available offline; it
  supports `/SILENT` and `/VERYSILENT`. Build it before the main installer:
  `ISCC.exe installer\emusic-presets.iss` (or pass `/DPresetsHelper=<path>`).
- **Hashes.** The download URLs and SHA-256 hashes live in
  `installer\presets-archives.iss`, generated by `scripts\fetch-presets.ps1`
  (the committed copy has empty placeholders). The
  `.github/workflows/presets.yml` workflow packs each optional pack, publishes
  it to a `viz-presets-<yyyymmdd>` GitHub release, regenerates that file and
  opens a pull request; the installer release then picks up the hashes.
- **Uninstall.** `presets.iss` adds `[UninstallDelete]` entries for the
  downloadable packs, so the uninstaller removes them too.

`scripts\fetch-presets.ps1` also accepts `-Pack`, `-OutDir`, `-ArchiveDir`,
`-ReleaseTag`, `-SkipDownload`, `-NoArchive`, `-NoUpdate` and `-Force`; run
`Get-Help scripts\fetch-presets.ps1 -Full` for the details.
