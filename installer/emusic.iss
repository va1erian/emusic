; emusic installer script (#29), written for Inno Setup 7.
;
; Per-user (no admin) install of the 64-bit emusic build into
; {localappdata}\Programs\emusic. BASS DLLs are never committed; they are
; picked up from the `bass\` folder next to the built exe when present, and
; the installer degrades to a BASS-less build with a compiler warning when
; they are absent (see docs/installer.md).
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

; BASS DLL folder. Defaults to the `bass\` folder next to the built exe,
; which is where the README tells developers to put the (never committed)
; x64 DLLs. Override with /DBassDir=... if you keep them elsewhere.
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
#if DirExists(BassDir)
Source: "{#BassDir}\*.dll"; DestDir: "{app}\bass"; Flags: ignoreversion skipifsourcedoesntexist
#else
  #pragma warning "BASS DLLs not found in the build's bass folder; building an installer without BASS. Place the x64 DLLs next to the built exe and rebuild - see docs/installer.md."
#endif
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
