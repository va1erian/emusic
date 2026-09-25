# Building the projectM visualization DLLs

The MilkDrop visualization is powered by [libprojectM](https://github.com/projectM-visualizer/projectm)
4.x (#295), which is **LGPL-2.1**. Like BASS, it is never linked at build time
and never committed: `crates/projectm` loads `projectM-4.dll`,
`projectM-4-playlist.dll` and (on 4.1) `glew32.dll` at runtime from a
`projectm\` folder next to the executable, or from `$EMUSIC_PROJECTM_DIR` when
that is set (`crates/projectm/src/ffi/loader.rs`). When the DLLs are absent the
frontend falls back to its placeholder, so the workspace builds and tests
without them.

This page covers building those DLLs with `scripts/build-projectm.ps1`, what CI
does with them, and how the installer ships them.

## Pinned revisions

| Dependency | Tag | Commit |
| --- | --- | --- |
| [libprojectM](https://github.com/projectM-visualizer/projectm) | `v4.1.7` | `e0b0a967f0ffd7d332106c366668ed271718472b` |
| [GLEW](https://github.com/nigels-com/glew) | `glew-2.2.0` | `9fb23c3e61cbd2d581e33ff7d8579b572b38ee26` |

libprojectM's tag and commit and GLEW's tag live in `scripts/build-projectm.ps1`.
libprojectM is cloned and the commit is verified after checkout, so a moved tag
cannot silently change the build; GLEW's git repo omits the generated `GL/*.h`
headers, so the script downloads its official release archive and verifies a
pinned SHA-256 instead. Bumping a revision means updating this table, the script
and [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md) together.

## Prerequisites

- Windows x64.
- [CMake](https://cmake.org/) 3.21 or newer on `PATH` (`winget install Kitware.CMake`).
- Visual Studio 2022+ (or the Build Tools) with the **Desktop development with
  C++** workload; the script finds it with `vswhere` and imports `vcvars64.bat`
  itself.
- `git` on `PATH`. The first run clones libprojectM, downloads GLEW's pinned
  release archive and builds both, which takes a few minutes.

## Building

From the repository root:

```
scripts\build-projectm.ps1
```

The output is `target\release\projectm\`:

- `projectM-4.dll` — the visualization engine;
- `projectM-4-playlist.dll` — the preset playlist API the crate uses;
- `glew32.dll` — the OpenGL loader projectM 4.1 links on Windows.

Pass `-Profile debug` to stage into `target\debug\projectm\` instead, or
`-OutputDir <path>` to copy elsewhere. Sources and intermediate builds live in
`target\projectm-build\`, so `cargo clean` removes them too; `-Force` re-clones.

The DLLs are never committed — `.gitignore` ignores `*.dll`, the same rule as
BASS.

## What the script does

libprojectM 4.1 on Windows includes `<GL/glew.h>` and `CMakeLists.txt` does
`find_package(GLEW REQUIRED)`, so GLEW is a real build and runtime dependency
despite the vendored glad used on other platforms. The script therefore:

1. clones libprojectM (with the `vendor/projectm-eval` submodule) at the pinned
   commit and downloads GLEW's pinned release archive;
2. builds GLEW (`build\cmake`, `BUILD_UTILS=OFF`, `/MT`) and installs it to a
   private prefix;
3. configures libprojectM with `BUILD_SHARED_LIBS=ON`, `ENABLE_PLAYLIST=ON`,
   `ENABLE_SDL_UI=OFF`, the vendored glm and projectm-eval (no other system
   dependencies) and `CMAKE_PREFIX_PATH` pointing at that GLEW prefix;
4. builds x64 Release with a static (`/MT`) CRT — `CMAKE_MSVC_RUNTIME_LIBRARY`
   for libprojectM and an explicit `/MT` for GLEW, whose CMake predates that
   variable — so the DLLs need no Visual C++ runtime, and copies the three DLLs
   out of `<prefix>\bin`.

`CMAKE_POLICY_VERSION_MINIMUM=3.5` is passed to GLEW so its 2.8-era
`cmake_minimum_required` still configures on CMake 4.

### Why not the vcpkg port

The upstream project publishes a [vcpkg manifest](https://github.com/projectM-visualizer/projectm/blob/v4.1.7/vcpkg.json)
and its own CI builds with `vcpkg` (which is how it pulls in GLEW). We build
from source instead so the exact tag/commit is pinned in-repo and CI needs no
vcpkg bootstrap, mirroring how the BASS DLLs are handled — and we use vcpkg's
own approach only for the one unavoidable dependency (GLEW). The trade-off is
that this script is the single source of truth for the projectM version.

## CI

`.github/workflows/projectm.yml` builds the DLLs with this script and caches
them under `target/release/projectm` (keyed by the script's contents, so a
pinned-version bump invalidates the cache). It runs on demand, when the script
or workflow changes, and as a reusable workflow: the release job calls it and
downloads the `projectm-dlls` artifact into `target/release/projectm` before
building the installers and portable zips.

## Installer

`installer/emusic.iss` includes `installer/projectm-dlls.iss`, which copies
`projectm\*.dll` to `{app}\projectm\` and the LGPL-2.1 text
(`COPYING-LGPL-2.1.txt`, committed at the repo root) next to them. Unlike BASS,
the DLLs are **optional** at packaging time: a missing `projectm\` folder is an
ISPP warning, not an error, because the app still runs with its visualization
placeholder. Override the folder with `/DProjectMDir=<path>`.

## Licensing

libprojectM is LGPL-2.1. The DLLs are built from unmodified upstream sources,
shipped with the full license text, and the source for the pinned tag is linked
above. They are loaded dynamically at runtime, so the application is a "work
that uses the Library": the user can drop in their own build of the same major
ABI by replacing the files in `projectm\`. GLEW is covered by its own
BSD/MIT-style license; see [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md).
