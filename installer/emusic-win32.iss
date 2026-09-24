; emusic (Win32) installer script (#240), written for Inno Setup 7.
;
; Companion to installer\emusic.iss: packages the native Win32 frontend
; (`emusic-win32.exe`, #91) as a *separate* per-user install so it can sit
; side by side with the egui build rather than replacing it. It has its own
; AppId, install dir, Start Menu shortcut and uninstall entry (see [Setup]
; below); nothing here touches the egui installer's identity.
;
; Both frontends read the same `%APPDATA%\emusic` config/library directory
; (see emusic_ui::config) and are packaged from the same BASS-required
; `bass\` folder and the same committed icon set (#124, #125) — see
; docs/installer.md for the shared-data note.
;
; File associations: both frontends register under the same "emusic" name
; (crates/app/src/main.rs and crates/win32/src/main.rs both call
; `AssocManager::new("emusic")`), so whichever one last registered owns the
; file-open association — there is only one active default player at a
; time. The egui installer registers unconditionally; this installer instead
; makes it an opt-in [Tasks] checkbox, unchecked by default, so installing
; the Win32 build alongside the egui build does not silently steal its file
; associations.
;
; Build from the repository root, after the egui installer's prerequisites
; (BASS DLLs in target\release\bass\, cargo build --release --workspace):
;   ISCC.exe installer\emusic-win32.iss
;
; Locations can be overridden without editing this file, e.g.:
;   ISCC.exe /DBuildDir=C:\path\to\release installer\emusic-win32.iss
;   ISCC.exe /DBassDir=C:\path\to\bass      installer\emusic-win32.iss

#define AppName "emusic (Win32)"
#define AppVersion "0.1.0"
#define AppPublisher "emusic"
#define AppURL "https://github.com/va1erian/emusic"
#define AppExeName "emusic-win32.exe"

; Directory produced by `cargo build --release --workspace` (same output dir
; as the egui build), resolved relative to this script (installer\) unless
; overridden with /DBuildDir=...
#ifndef BuildDir
  #define BuildDir AddBackslash(SourcePath) + "..\target\release"
#endif

; BASS DLL folder, a required input (#124), shared with the egui installer.
; Defaults to the `bass\` folder next to the built exes. Override with
; /DBassDir=... if you keep them elsewhere; the folder must be non-empty or
; the compile fails.
#ifndef BassDir
  #define BassDir AddBackslash(BuildDir) + "bass"
#endif

; Icon set (#125), shared with the egui installer. Committed at the
; repository root, so it is resolved relative to this script (installer\) by
; default. Override with /DAssetsDir=... if you keep it elsewhere.
#ifndef AssetsDir
  #define AssetsDir AddBackslash(SourcePath) + "..\assets"
#endif

; Require Inno Setup 7: SetupArchitecture (7.0) and the changed AppVerName
; default are used below. `VER` is the compiler version.
#if VER < EncodeVer(7, 0, 0)
  #error Requires Inno Setup 7 or newer (this script uses SetupArchitecture).
#endif

; Fail early with an actionable message instead of a cryptic "file not found"
; later on.
#if FileExists(AddBackslash(BuildDir) + AppExeName)
#else
  #error emusic-win32.exe not found in the release build folder. Run `cargo build --release --workspace` first, or pass /DBuildDir=<path>.
#endif

; BASS is required at packaging time (#124): emusic-win32 needs the x64 BASS
; DLLs at runtime, so fail the compile with an actionable message rather than
; shipping an installer that cannot play audio. The `bass*.dll` mask catches
; the runtime (bass.dll) plus the codec add-ons the app auto-loads, and also
; an empty `bass\` folder.
#if DirExists(BassDir) && FindFirst(AddBackslash(BassDir) + "bass*.dll", 0)
#else
  #error BASS DLLs not found in the bass folder. Put the x64 BASS DLLs (bass.dll plus any codec add-ons) in the build's bass folder, or pass /DBassDir=<path> - see docs/installer.md.
#endif

[Setup]
; Distinct from the egui installer's AppId (B35420CE-DB33-4E58-8CDE-B5D80F6E0002)
; so the two installers never see each other as the same product: no shared
; upgrade/uninstall entry, no accidental replace.
AppId={{317E9C4F-71F7-49A6-835B-252A2D803D16}
AppName={#AppName}
AppVersion={#AppVersion}
; Inno Setup 7's default AppVerName is now "<AppName> <AppVersion>", so it is
; intentionally left unset here instead of spelling out the pre-7
; "<AppName> version <AppVersion>" form.
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}
AppUpdatesURL={#AppURL}
; Distinct install dir from the egui build's {localappdata}\Programs\emusic,
; so both can be installed at once.
DefaultDirName={localappdata}\Programs\emusic-win32
DisableProgramGroupPage=yes
; Per-user install: never request elevation, touch nothing machine-wide.
PrivilegesRequired=lowest
OutputBaseFilename=emusic-win32-{#AppVersion}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern dynamic
; Inno Setup 7 directive: build a real 64-bit installer (high-entropy ASLR,
; larger LZMA dictionary). It already defaults the two architecture directives
; below to x64compatible; they are spelled out to document the intent: the app
; and the BASS DLLs are x64, so only 64-bit Windows can run this.
SetupArchitecture=x64
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayName={#AppName}
; Installer's own icon, and the icon Add/Remove Programs shows for emusic
; (Win32) (#125). The uninstall icon is one of the per-extension files
; copied below, shared with the egui installer.
SetupIconFile={#AssetsDir}\ico\emusic-setup.ico
UninstallDisplayIcon={app}\icons\emusic-uninstall.ico
VersionInfoVersion=0.1.0.0
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} installer
VersionInfoCompany={#AppPublisher}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
; Unchecked by default (#240): both frontends register under the same
; "emusic" association name, so enabling this hands file-open ownership to
; the Win32 build and away from whichever frontend last registered it.
Name: "fileassoc"; Description: "Make {#AppName} the default player for supported audio files (replaces any association set by the standard emusic installer)"; Flags: unchecked

[Files]
Source: "{#BuildDir}\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion
; BASS is required (#124): the early #error above guarantees this matches at
; least bass.dll, so no skipifsourcedoesntexist fallback is needed.
Source: "{#BassDir}\*.dll"; DestDir: "{app}\bass"; Flags: ignoreversion
#if DirExists(AddBackslash(AssetsDir) + "ico")
Source: "{#AssetsDir}\ico\*.ico"; DestDir: "{app}\icons"; Flags: ignoreversion
#else
  #pragma warning "Icon files not found in the assets folder; building an installer without file-association icons. Pass /DAssetsDir=<path> if they live elsewhere."
#endif

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExeName}"; WorkingDir: "{app}"; Comment: "emusic music player (native Win32 frontend)"

[Run]
; Register per-user file associations only if the fileassoc task was checked
; (#240) — unlike the egui installer, which always registers. The app exits
; by itself, so no nowait is needed.
Filename: "{app}\{#AppExeName}"; Parameters: "--register-associations"; Flags: runhidden; StatusMsg: "Registering file associations..."; Tasks: fileassoc

[UninstallRun]
; Always attempt to unregister on uninstall, even if the task was never
; checked; a no-op unregister is harmless and this way a user who enabled the
; task later, or another emusic install that overwrote it, is still cleaned
; up. Runs before the files are removed, while emusic-win32.exe is still
; present.
Filename: "{app}\{#AppExeName}"; Parameters: "--unregister"; Flags: runhidden; RunOnceId: "UnregisterAssociationsWin32"

[UninstallDelete]
; The attribution notice is written by [Code] below, so the uninstaller does
; not know about it and must remove it explicitly.
Type: files; Name: "{app}\bass\README.txt"

[Code]
// [Code] is real Pascal Script, not the INI-style sections above it: use //
// or { } here, never ";" — a ";" comment silently breaks the parser instead
// of being skipped (confirmed against Inno Setup 7.1.0; reported as a
// misleading "'BEGIN' expected" pointing at an unrelated later line).
//
// Attribution bundled with the BASS DLLs (#124). Not required by BASS's
// free-for-non-commercial license, but good practice; the names stay the
// property of their owners.
const
  BassNotice = 'Audio playback uses the BASS library by un4seen developments' + #13#10 +
    '(https://www.un4seen.com). BASS is free for non-commercial use; see the' + #13#10 +
    'bass.txt license files in the BASS download. A commercial build needs its' + #13#10 +
    'own BASS license. BASS and un4seen remain the property of their owners.';

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    SaveStringToFile(ExpandConstant('{app}\bass\README.txt'), BassNotice, False);
end;
