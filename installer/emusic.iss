; emusic installer script (#29), written for Inno Setup 7.
;
; Per-user (no admin) install of the 64-bit emusic build into
; {localappdata}\Programs\emusic. BASS is a required packaging input (#124):
; the DLLs are never committed to git, but they must be present in the `bass\`
; folder next to the built exe, and the compile fails with an #error when the
; folder is missing or has no `bass*.dll` (see docs/installer.md).
;
; The icon set (#125) is committed under assets\ and installed into an
; `icons\` subfolder of the install dir, where emusic.exe's file-association
; registration looks for the per-extension icons.
;
; Build from the repository root:
;   cargo build --release
;   ISCC.exe installer\emusic.iss      (path to Inno Setup 7's command-line compiler)
;
; Locations can be overridden without editing this file, e.g.:
;   ISCC.exe /DBuildDir=C:\path\to\release installer\emusic.iss
;   ISCC.exe /DBassDir=C:\path\to\bass      installer\emusic.iss

#define AppName "emusic"
#define AppVersion "0.1.0"
#define AppPublisher "emusic"
#define AppURL "https://github.com/va1erian/emusic"
#define AppExeName "emusic.exe"

; Directory produced by `cargo build --release`, resolved relative to this
; script (installer\) unless overridden with /DBuildDir=...
#ifndef BuildDir
  #define BuildDir AddBackslash(SourcePath) + "..\target\release"
#endif

; BASS DLL folder, a required input (#124). Defaults to the `bass\` folder
; next to the built exe, which is where the README tells developers to put the
; (never committed) x64 DLLs. Override with /DBassDir=... if you keep them
; elsewhere; the folder must be non-empty or the compile fails.
#ifndef BassDir
  #define BassDir AddBackslash(BuildDir) + "bass"
#endif

; Icon set (#125). Committed at the repository root, so it is resolved
; relative to this script (installer\) by default. Override with
; /DAssetsDir=... if you keep it elsewhere.
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
  #error emusic.exe not found in the release build folder. Run `cargo build --release` first, or pass /DBuildDir=<path>.
#endif

; BASS is required at packaging time (#124): emusic needs the x64 BASS DLLs at
; runtime, so fail the compile with an actionable message rather than shipping
; an installer that cannot play audio. The `bass*.dll` mask catches the
; runtime (bass.dll) plus the codec add-ons the app auto-loads, and also an
; empty `bass\` folder.
#if DirExists(BassDir) && FindFirst(AddBackslash(BassDir) + "bass*.dll", 0)
#else
  #error BASS DLLs not found in the bass folder. Put the x64 BASS DLLs (bass.dll plus any codec add-ons) in the build's bass folder, or pass /DBassDir=<path> - see docs/installer.md.
#endif

[Setup]
; Stable identity for upgrades/uninstall; the double brace yields a literal '{'.
AppId={{B35420CE-DB33-4E58-8CDE-B5D80F6E0002}
AppName={#AppName}
AppVersion={#AppVersion}
; Inno Setup 7's default AppVerName is now "<AppName> <AppVersion>", so it is
; intentionally left unset here instead of spelling out the pre-7
; "<AppName> version <AppVersion>" form.
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}
AppUpdatesURL={#AppURL}
DefaultDirName={localappdata}\Programs\emusic
DisableProgramGroupPage=yes
; Per-user install: never request elevation, touch nothing machine-wide.
PrivilegesRequired=lowest
OutputBaseFilename=emusic-{#AppVersion}-setup
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
; (#125). The uninstall icon is one of the per-extension files copied below.
SetupIconFile={#AssetsDir}\ico\emusic-setup.ico
UninstallDisplayIcon={app}\icons\emusic-uninstall.ico
VersionInfoVersion=0.1.0.0
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} installer
VersionInfoCompany={#AppPublisher}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

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
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExeName}"; WorkingDir: "{app}"; Comment: "emusic music player"

[Run]
; Register per-user file associations once setup finished, in silent installs
; too. The app exits by itself, so no nowait is needed.
Filename: "{app}\{#AppExeName}"; Parameters: "--register-associations"; Flags: runhidden; StatusMsg: "Registering file associations..."

[UninstallRun]
; Runs before the files are removed, while emusic.exe is still present.
Filename: "{app}\{#AppExeName}"; Parameters: "--unregister"; Flags: runhidden; RunOnceId: "UnregisterAssociations"

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
