; emusic "get more presets" helper (#299), written for Inno Setup 7.
;
; A tiny stand-alone installer that adds the optional projectM preset packs to
; an existing emusic install. The main installer bundles this helper into
; {app}\ so it is always available offline; running it again later lets the
; user add packs without downloading the whole emusic setup again. The app
; itself never downloads anything.
;
; It reads the install directory from the installer's per-user uninstall key
; (AppId, see installer\emusic.iss) and reuses the shared download/extraction
; code in installer\presets.iss. Supports Inno Setup's /SILENT and /VERYSILENT
; for scripted installs.
;
; Build (after building the main installer or stand-alone, no emusic.exe
; needed):
;   ISCC.exe installer\emusic-presets.iss
;
; Overrides:
;   ISCC.exe /DAssetsDir=C:\path\to\assets installer\emusic-presets.iss

#define AppName "emusic presets"
#define AppVersion "0.1.0"
#define AppPublisher "emusic"
#define AppURL "https://github.com/va1erian/emusic"

; Icon set (#125), shared with the other installers.
#ifndef AssetsDir
  #define AssetsDir AddBackslash(SourcePath) + "..\assets"
#endif

; The shared download/extract code also requires ExtractArchive (Inno Setup
; 6.4+); this script already targets 7 like the other emusic installers.
#if VER < EncodeVer(7, 0, 0)
  #error Requires Inno Setup 7 or newer.
#endif

#include "presets.iss"

[Setup]
; Stable identity so re-running the helper upgrades itself in place. Distinct
; from both frontend installers.
AppId={{67C19AD9-47FC-4DEA-B505-93AA9D15667D}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}
AppUpdatesURL={#AppURL}
; The main installer's directory, read from its uninstall key; the user must
; not change it or packs would land outside the install.
DefaultDirName={code:PresetsInstallDir}
DisableDirPage=yes
DisableProgramGroupPage=yes
; The helper lives inside {app} and is removed by the main uninstaller, so it
; must not register an uninstaller or Add/Remove Programs entry of its own.
Uninstallable=no
CreateUninstallRegKey=no
PrivilegesRequired=lowest
OutputBaseFilename=emusic-presets-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern dynamic
#if FileExists(AddBackslash(AssetsDir) + "ico\emusic-setup.ico")
SetupIconFile={#AssetsDir}\ico\emusic-setup.ico
#endif
VersionInfoVersion=0.1.0.0
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} helper
VersionInfoCompany={#AppPublisher}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Code]
var
  PacksPage: TInputOptionWizardPage;

// The install directory to add packs to. Only used when the uninstall key is
// present (InitializeSetup blocks otherwise); the fallback keeps the constant
// valid while the wizard starts.
function PresetsInstallDir(Param: String): String;
begin
  Result := PresetsFindInstallDir;
  if Result = '' then
    Result := ExpandConstant('{localappdata}\Programs\emusic');
end;

function InitializeSetup: Boolean;
begin
  Result := True;
  if PresetsFindInstallDir = '' then begin
    MsgBox('emusic does not appear to be installed for this user. ' +
      'Install it first, then run this tool again.', mbError, MB_OK);
    Result := False;
  end;
end;

procedure InitializeWizard;
begin
  PacksPage := PresetsCreatePage(wpSelectDir);
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Names: TStringList;
begin
  if CurStep = ssPostInstall then begin
    Names := TStringList.Create;
    try
      PresetsCollectChecked(PacksPage, Names);
      PresetsInstallSelected(Names);
    finally
      Names.Free;
    end;
  end;
end;
