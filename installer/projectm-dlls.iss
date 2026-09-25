; libprojectM visualization DLLs (#298), included by installer\emusic.iss.
;
; The MilkDrop visualization is powered by libprojectM 4.x and its GLEW loader
; (LGPL-2.1; see docs\projectm.md and THIRD-PARTY-NOTICES.md). Like BASS, the
; DLLs are never committed; scripts\build-projectm.ps1 stages them into
; `projectm\` next to the built exe, and [Files] below copies them to
; {app}\projectm\ with the committed LGPL text.
;
; Unlike BASS the DLLs are optional at packaging time: the app falls back to
; its visualization placeholder, so a missing folder is only a warning, not an
; #error. Override the folder with /DProjectMDir=<path>.

#ifndef ProjectMDir
  #define ProjectMDir AddBackslash(BuildDir) + "projectm"
#endif

; LGPL-2.1 text for libprojectM, committed at the repo root and resolved
; relative to installer\emusic.iss by default.
#ifndef LgplNotice
  #define LgplNotice AddBackslash(SourcePath) + "..\COPYING-LGPL-2.1.txt"
#endif

[Files]
; The check for projectM-4.dll also proves the folder is non-empty; `*.dll`
; then picks up that core library, the playlist library and the glew32.dll
; projectM 4.1 links against.
#if DirExists(ProjectMDir) && FindFirst(AddBackslash(ProjectMDir) + "projectM-4.dll", 0)
Source: "{#ProjectMDir}\*.dll"; DestDir: "{app}\projectm"; Flags: ignoreversion
; The LGPL-2.1 text ships with the unmodified DLLs; it is committed, so it is
; always present when the DLLs are.
Source: "{#LgplNotice}"; DestDir: "{app}\projectm"; Flags: ignoreversion
#else
  #pragma warning "projectM DLLs not found in the projectm folder; the visualization will fall back to its placeholder. Run scripts\build-projectm.ps1, or pass /DProjectMDir=<path>."
#endif
