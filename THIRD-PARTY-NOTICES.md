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

## Visualization presets (projectM)

The MilkDrop presets that projectM renders come from the
[projectM-visualizer](https://github.com/projectM-visualizer) preset
repositories, pinned by commit. Most of the original preset collection is
redistributed under the projectM project's own practice (see *Licensing*
below); the pinned commits are listed so every file can be traced.

| Pack | Files | Delivery | Source repository | Pinned commit |
|---|---|---|---|---|
| milkdrop-original | 552 presets | bundled | `projectM-visualizer/presets-milkdrop-original` | `e03b83e3338d8f1ed6cbcf908c719f249ef24288` |
| milkdrop-texture-pack | 67 textures | bundled | `projectM-visualizer/presets-milkdrop-texture-pack` | `6368812f27bc747b517218fbf89d21d59afce4d9` |
| cream-of-the-crop | 9,795 presets | optional download | `projectM-visualizer/presets-cream-of-the-crop` | `0180df21f5e0bd39b9060cc5de420ed2f1f9e509` |
| en-d | 40 presets | optional download | `projectM-visualizer/presets-en-d` | `fff71ea81223109f3558351667eef851f2781c96` |
| projectm-classic | 4,188 presets | optional download | `projectM-visualizer/presets-projectm-classic` | `14a6244a7d32eb7e114e1a92d1cb93358cdcc54a` |

The optional packs are downloaded only when the user asks for them, by the
Inno Setup installers (`installer/presets.iss`); the app itself contains no
download code. The archives are published on the emusic GitHub releases and
verified by SHA-256 before they are extracted.

### Licensing

MilkDrop presets were, in almost all cases, not released under any specific
license. The individual preset authors theoretically hold the copyright, but
because the presets were released freely and have been used by many packages
and applications for two decades, the projectM team treats them as public
domain. The `presets-cream-of-the-crop` and `presets-en-d` repositories state
this explicitly in their `LICENSE.md`; emusic redistributes the collection on
the same terms as the projectM project.

The `milkdrop-original` presets shipped with Nullsoft MilkDrop 2, whose source
was released under the BSD license. The `milkdrop-texture-pack` contains the
original MilkDrop textures plus textures used by many presets.

If a preset author does not want their creation included, the projectM team
removes it from future releases on request. Follow the same policy for emusic:
open an issue at <https://github.com/va1erian/emusic/issues> and the preset
will be removed from the next release.
