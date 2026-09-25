; projectM visualization presets, shared by the installers (#299).
;
; The full preset collection is ~140 MB, so the installers only bundle the
; small base set (milkdrop-original + the texture pack) and offer the large
; packs as an optional download. This file holds the download/extract code
; and the Windows-side pack table; the download URLs and SHA-256 hashes are
; generated into `presets-archives.iss` by `scripts\fetch-presets.ps1`.
;
; Included by:
;   installer\emusic.iss          (at install time, via its [Components])
;   installer\emusic-presets.iss  (the stand-alone "get more presets" helper)
;
; It is meant to be #included at the top level of the script (before [Setup]),
; so its own [Setup], [UninstallDelete] and [Code] sections are merged in.
; Nothing is downloaded by the app itself; this is Inno Setup only.
;
; ExtractArchive was added in Inno Setup 6.4 and is why the shared code needs
; at least that version; the emusic installer already requires Inno Setup 7
; (see installer\emusic.iss and docs\installer.md), so 7 is the effective
; minimum. `ArchiveExtraction=enhanced/nopassword` keeps the memory use of the
; big .7z packs sane and does not need password support.

[Setup]
ArchiveExtraction=enhanced/nopassword

[UninstallDelete]
; Downloaded packs are created by [Code], so the uninstaller does not know
; about them and must remove them explicitly. The bundled base set is handled
; by its [Files] entries in emusic.iss and is left out here.
Type: filesandordirs; Name: "{app}\visualizations\presets\cream-of-the-crop"
Type: filesandordirs; Name: "{app}\visualizations\presets\en-d"
Type: filesandordirs; Name: "{app}\visualizations\presets\projectm-classic"

[Code]
#include "presets-archives.iss"

const
  // The downloadable packs, in the same order as the generated accessors in
  // presets-archives.iss and as the [Components] entries in
  // installer\emusic.iss. Every pack installs under
  // {app}\visualizations\presets\<name>, which is where the app looks for
  // preset packs (emusic_ui::state::projectm).
  PresetPackCount = 3;

  // Registry keys the installer writes its install location to. It uses
  // PrivilegesRequired=lowest, so the uninstall key is per-user under HKCU.
  // The primary AppId matches installer\emusic.iss (the double brace there is
  // a literal "{"); the legacy one is the removed `emusic-win32` frontend
  // (#317) so the helper still finds an older install.
  PresetEmusicUninstallKey =
    'Software\Microsoft\Windows\CurrentVersion\Uninstall\' +
    '{B35420CE-DB33-4E58-8CDE-B5D80F6E0002}_is1';
  PresetLegacyEmusicUninstallKey =
    'Software\Microsoft\Windows\CurrentVersion\Uninstall\' +
    '{317E9C4F-71F7-49A6-835B-252A2D803D16}_is1';

{ The install directory recorded by the main installer for this user, or ''
  when it is not installed. }
function PresetsFindInstallDir: String;
var
  Value: String;
begin
  Result := '';
  if RegQueryStringValue(HKEY_CURRENT_USER, PresetEmusicUninstallKey,
    'InstallLocation', Value) and (Value <> '') then
    Result := Value
  else if RegQueryStringValue(HKEY_CURRENT_USER, PresetLegacyEmusicUninstallKey,
    'InstallLocation', Value) and (Value <> '') then
    Result := Value;
end;

{ Folder name of pack `Index`, the key the app and the config use. }
function PresetPackNameOf(Index: Integer): String;
begin
  case Index of
    0: Result := 'cream-of-the-crop';
    1: Result := 'en-d';
    2: Result := 'projectm-classic';
  else
    Result := '';
  end;
end;

{ Human-readable label with the pack's size, shown as a checkbox caption. }
function PresetPackTitleOf(Index: Integer): String;
begin
  case Index of
    0: Result := 'cream-of-the-crop - 9,795 presets, 111 MB';
    1: Result := 'en-d - 40 presets, 2.8 MB';
    2: Result := 'projectm-classic - 4,188 presets, 24.6 MB (overlaps the other packs)';
  else
    Result := '';
  end;
end;

{ Suffix of the matching [Components] entry ("vizpresets\<suffix>"), so the
  main installer can map its checkboxes back to a pack. }
function PresetPackComponentOf(Index: Integer): String;
begin
  case Index of
    0: Result := 'cream';
    1: Result := 'end';
    2: Result := 'classic';
  else
    Result := '';
  end;
end;

{ projectM-classic is unchecked by default: it overlaps the other packs
  heavily. The others start checked, matching the issue's suggestion. }
function PresetPackDefaultCheckedOf(Index: Integer): Boolean;
begin
  case Index of
    0: Result := True;
    1: Result := True;
    2: Result := False;
  else
    Result := False;
  end;
end;

{ Index of the pack with this folder name, or -1. }
function PresetsIndexOf(const Name: String): Integer;
var
  I: Integer;
begin
  Result := -1;
  for I := 0 to PresetPackCount - 1 do
    if CompareText(PresetPackNameOf(I), Name) = 0 then begin
      Result := I;
      Exit;
    end;
end;

{ The folder a pack is installed into. }
function PresetsPackFolder(const Name: String): String;
begin
  Result := ExpandConstant('{app}\visualizations\presets\') + Name;
end;

{ Whether a pack's files are already present, so the helper can pre-check it
  and offer to reinstall. }
function PresetsPackInstalled(const Name: String): Boolean;
begin
  Result := DirExists(PresetsPackFolder(Name));
end;

{ Whether the pack has a pinned archive URL; false for a local/dev build whose
  presets-archives.iss still holds placeholders. }
function PresetsPackDownloadable(Index: Integer): Boolean;
begin
  Result := (Index >= 0) and (Index < PresetPackCount) and
    (PresetArchiveUrlOf(Index) <> '');
end;

{ Creates the check-box page offered by the stand-alone helper. Packs that are
  already installed start checked, so re-running the helper can repair them. }
function PresetsCreatePage(const AfterID: Integer): TInputOptionWizardPage;
var
  I: Integer;
begin
  Result := CreateInputOptionPage(AfterID, 'Visualization presets',
    'Choose the optional projectM preset packs to install.',
    'Packs are downloaded from the emusic GitHub releases and verified by ' +
    'SHA-256. The bundled presets keep working if a download fails.',
    False, False);
  for I := 0 to PresetPackCount - 1 do begin
    Result.Add(PresetPackTitleOf(I));
    Result.Values[I] :=
      PresetsPackInstalled(PresetPackNameOf(I)) or
      PresetPackDefaultCheckedOf(I);
  end;
end;

{ Appends the folder names of the checked packs on the helper's page. }
procedure PresetsCollectChecked(Page: TInputOptionWizardPage;
  const Names: TStringList);
var
  I: Integer;
begin
  for I := 0 to PresetPackCount - 1 do
    if Page.Values[I] then
      Names.Add(PresetPackNameOf(I));
end;

{ Downloads and extracts the named packs. Failures are non-fatal: each pack is
  tried independently and a summary is shown at the end, so a network problem
  never aborts an install. }
procedure PresetsInstallSelected(const Names: TStringList);
var
  Page: TDownloadWizardPage;
  I, J, Downloadable: Integer;
  Url, Sha, BaseName, TempFile, Failures: String;
begin
  Page := nil;
  Failures := '';
  Downloadable := 0;
  for J := 0 to Names.Count - 1 do begin
    I := PresetsIndexOf(Names[J]);
    if not PresetsPackDownloadable(I) then begin
      if I >= 0 then
        Log(Format('Preset pack %s has no archive URL in this build; skipping.', [PresetPackNameOf(I)]));
      Continue;
    end;
    Downloadable := Downloadable + 1;
    if Page = nil then begin
      Page := CreateDownloadPage('Downloading visualization presets',
        'The optional preset packs are only downloaded if you selected them.',
        nil);
      Page.ShowBaseNameInsteadOfUrl := True;
      Page.Show;
      Log(Format('Downloading preset packs from release %s.', [PresetReleaseTag]));
    end;
    Url := PresetArchiveUrlOf(I);
    Sha := PresetArchiveSha256Of(I);
    BaseName := PresetPackNameOf(I) + '.7z';
    Page.Clear;
    Page.Add(Url, BaseName, Sha);
    try
      Page.Download;
      TempFile := ExpandConstant('{tmp}\') + BaseName;
      // The archive stores visualizations\presets\<pack>\..., so extracting
      // into {app} lands it next to the bundled base set.
      ExtractArchive(TempFile, ExpandConstant('{app}'), '', True, nil);
      Log(Format('Installed visualization preset pack %s.', [PresetPackNameOf(I)]));
    except
      Failures := Failures + PresetPackTitleOf(I) + ': ' +
        AddPeriod(GetExceptionMessage) + #13#10;
    end;
  end;
  if Page <> nil then
    Page.Hide;
  if Failures <> '' then
    SuppressibleMsgBox(
      'Some visualization presets could not be installed. The bundled ' +
      'presets still work; run the helper again to retry.' + #13#10 + #13#10 +
      Failures, mbError, MB_OK, IDOK)
  else if (Names.Count > 0) and (Downloadable = 0) then
    SuppressibleMsgBox(
      'This build of the presets tool has no download URLs (developer build), ' +
      'so nothing was installed.', mbInformation, MB_OK, IDOK);
end;

{ Entry point for the main installer: installs whichever packs were selected
  on its [Components] page. }
procedure PresetsInstallFromComponents;
var
  Names: TStringList;
  I: Integer;
begin
  Names := TStringList.Create;
  try
    for I := 0 to PresetPackCount - 1 do
      if WizardIsComponentSelected('vizpresets\' + PresetPackComponentOf(I)) then
        Names.Add(PresetPackNameOf(I));
    if Names.Count > 0 then
      PresetsInstallSelected(Names);
  finally
    Names.Free;
  end;
end;
