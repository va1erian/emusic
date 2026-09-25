# Handoff: projectM crate (#297) and the #295 plan

Status when handed off (2026-09-25):

- #300 (emusic-ui state/settings/commands) is done: PR #310 on `feat/300-viz-state`.
- #297 (this crate) is written and checked; this branch.

## What was verified
- `cargo fmt`, `cargo clippy --all-targets -D warnings` (Windows GNU target),
  unit tests under Wine.
- End to end against a real libprojectM 4.1.7 + GLEW 2.2 (cross-built with
  MinGW, run under Wine + Xvfb + Mesa, GL 4.5 core): load, version, GLEW init,
  create, texture paths, 552 milkdrop-original presets, direct preset load,
  playlist `play_index`/`play_next`, 60 rendered frames (non-black pixels),
  `PresetSwitched` events, `destroy` refused without a context and succeeding
  with it.
- The throwaway WGL test harness was not committed.

## Findings for #298 (building the DLLs)
- projectM 4.1 on Windows links **GLEW** (shared `glew32.dll` preferred); the
  crate calls `glewInit` (with `glewExperimental`) before `projectm_create`.
  Ship `glew32.dll` in `projectm\`.
- With MinGW, a shared libstdc++ crashed in `std::locale::operator=` on the
  first preset load; linking `-static-libstdc++ -static-libgcc` fixed it.
  With MSVC, prefer the static CRT (`/MT`) or ship the VC++ runtime.
- MinGW names the DLLs `libprojectM-4*.dll`; the crate expects the MSVC names
  `projectM-4.dll` / `projectM-4-playlist.dll`.

## Next steps
1. Open/finish the PR for this branch (Closes #297), let CI run.
2. #301 (Win32 widget) once the win32ui prerequisites (#296) land. Consider the
   UTF-8 active code page in `emusic-win32.manifest` so non-ASCII preset paths
   work (projectM opens files with narrow APIs).
3. #298/#299 packaging scripts (need Windows/MSVC/Inno Setup).
