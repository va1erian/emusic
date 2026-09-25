# Third-party notices

emusic is MIT-licensed (see [LICENSE](LICENSE)). It also ships or downloads
third-party components; this file records their licenses and sources. The
DLLs themselves are never committed to git.

## libprojectM

- **Component:** `projectM-4.dll`, `projectM-4-playlist.dll` (MilkDrop visualization engine)
- **Version:** 4.1.7
- **Source:** <https://github.com/projectM-visualizer/projectm/tree/v4.1.7> (commit `e0b0a967f0ffd7d332106c366668ed271718472b`)
- **License:** LGPL-2.1-only — the full text ships as
  [COPYING-LGPL-2.1.txt](COPYING-LGPL-2.1.txt) and next to the DLLs in
  `projectm\`.
- **Distribution:** built unmodified by `scripts/build-projectm.ps1` and loaded
  at runtime from `projectm\` next to the executable. The user may replace
  these files with their own build of the same major ABI; see
  [docs/projectm.md](docs/projectm.md).

## GLEW (OpenGL Extension Wrangler)

- **Component:** `glew32.dll` (the OpenGL loader libprojectM 4.1 links on Windows)
- **Version:** 2.2.0
- **Source:** <https://github.com/nigels-com/glew/tree/glew-2.2.0> (commit `9fb23c3e61cbd2d581e33ff7d8579b572b38ee26`)
- **License:** BSD/MIT-style (GLEW, Mesa 3-D, Khronos); see the project's
  `LICENSE.txt`. Built by `scripts/build-projectm.ps1`.
