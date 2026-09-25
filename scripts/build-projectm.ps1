<#
.SYNOPSIS
    Builds libprojectM (and the GLEW it links on Windows) from pinned sources
    with CMake + MSVC and stages the runtime DLLs for the visualizer (#298).

.DESCRIPTION
    Produces, into `target\<profile>\projectm\`:

      - projectM-4.dll           the MilkDrop engine (LGPL-2.1)
      - projectM-4-playlist.dll  preset playlist management (LGPL-2.1)
      - glew32.dll               the OpenGL loader projectM 4.1 links against

    libprojectM is cloned at a pinned tag and commit; GLEW is downloaded as the
    official release archive pinned by SHA-256 (its git repo omits the generated
    GL headers). Both go into `target\projectm-build\`, are built x64 Release
    with the static CRT (`/MT`, so the DLLs need no Visual C++ runtime) using
    the newest installed Visual Studio generator, and only those three DLLs are
    copied out.

    The DLLs are never committed (`*.dll` is git-ignored, the same rule as
    BASS). The installer treats a missing `projectm\` folder as a warning: the
    app falls back to its placeholder. See docs\projectm.md for how the app,
    the installer and CI consume the output.

.PARAMETER Profile
    Cargo profile whose `target\<profile>\projectm` folder receives the DLLs.
    Defaults to `release` (what the installer and CI use).

.PARAMETER OutputDir
    Where to copy the DLLs. Defaults to `<repo>\target\<profile>\projectm`.

.PARAMETER WorkDir
    Where the pinned sources are cloned and built. Defaults to
    `<repo>\target\projectm-build`, so `cargo clean` removes it too.

.PARAMETER Force
    Delete and re-clone the sources before building.

.EXAMPLE
    scripts\build-projectm.ps1

.EXAMPLE
    scripts\build-projectm.ps1 -Profile debug -OutputDir C:\tmp\projectm
#>
[CmdletBinding()]
param(
    [ValidateSet('debug', 'release')]
    [string]$Profile = 'release',
    [string]$OutputDir,
    [string]$WorkDir,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# --- Pinned revisions (#298) ------------------------------------------------
# Bumping either revision means updating the version/source in
# docs\projectm.md, installer\projectm-dlls.iss and THIRD-PARTY-NOTICES.md too.
$ProjectMRepo = 'https://github.com/projectM-visualizer/projectm.git'
$ProjectMTag = 'v4.1.7'
$ProjectMCommit = 'e0b0a967f0ffd7d332106c366668ed271718472b'
# GLEW's git repo intentionally omits the generated include\GL\*.h headers, so
# its *release archive* (which has them) is downloaded and hash-checked instead
# of cloned. THIRD-PARTY-NOTICES.md records the commit the tag points at.
$GlewTag = 'glew-2.2.0'
$GlewArchiveUrl = "https://github.com/nigels-com/glew/releases/download/$GlewTag/$GlewTag.zip"
$GlewArchiveSha256 = 'A9046A913774395A095EDCC0B0AC2D81C3AACCA61787B39839B941E9BE14E0D4'

if ($env:OS -ne 'Windows_NT') {
    throw 'This script builds Windows x64 DLLs and must run on Windows.'
}

$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not $OutputDir) { $OutputDir = Join-Path $repoRoot "target\$Profile\projectm" }
if (-not $WorkDir) { $WorkDir = Join-Path $repoRoot 'target\projectm-build' }

# Native tools (git, cmake, vcvars) write progress to stderr. A caller that
# pipes this script makes Windows PowerShell 5.1 turn that into a terminating
# error under `Stop`, so each call relaxes the preference and checks the exit
# code itself.
function Invoke-Native([string]$Name, [string[]]$Arguments) {
    Write-Host "$Name $($Arguments -join ' ')" -ForegroundColor DarkGray
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    & $Name @Arguments
    $code = $LASTEXITCODE
    $ErrorActionPreference = $previous
    if ($code -ne 0) { throw "$Name $($Arguments -join ' ') failed with exit code $code" }
}

# Adds the newest Visual Studio C++ x64 tools to this session's environment so
# CMake and Ninja/the VS generator find cl.exe, link.exe and the Windows SDK.
function Import-MsvcEnvironment {
    if (Get-Command cl.exe -ErrorAction SilentlyContinue) { return }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) {
        throw 'vswhere.exe not found. Install Visual Studio 2022+ (or the Build Tools) with the "Desktop development with C++" workload.'
    }
    $vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vsPath) { throw 'No Visual Studio installation with the C++ x64 tools found.' }
    $vcvars = Join-Path $vsPath 'VC\Auxiliary\Build\vcvars64.bat'
    if (-not (Test-Path $vcvars)) { throw "vcvars64.bat not found at $vcvars" }
    $envLines = & cmd /c "`"$vcvars`" >nul 2>&1 && set"
    foreach ($line in $envLines) {
        if ($line -match '^([^=]+)=(.*)$') {
            Set-Item -Path "Env:$($matches[1])" -Value $matches[2]
        }
    }
}

# The VS generator name (e.g. "Visual Studio 17 2022") from the newest
# installed Visual Studio, so the script does not pin a VS year. `cmake --help`
# marks the default generator with a leading `*` (the newest supported one);
# fall back to the first match, since the list is ordered newest first.
function Get-VsGenerator {
    $help = (& cmake --help) -join "`n"
    $default = [regex]::Match($help, '^\s*\*\s*(Visual Studio 1\d 20\d\d)', 'Multiline')
    if ($default.Success) { return $default.Groups[1].Value }
    $any = [regex]::Match($help, 'Visual Studio 1\d 20\d\d')
    if (-not $any.Success) {
        throw 'No Visual Studio generator in `cmake --help`. Install Visual Studio 2022+ with the C++ x64 tools.'
    }
    return $any.Value
}

# Clones `repo` into `dest` and checks out a pinned commit (with submodules,
# libprojectM vendors projectm-eval as one). Reuses the clone on re-runs.
function Get-PinnedSource([string]$Name, [string]$Repo, [string]$Commit) {
    $dest = Join-Path $WorkDir $Name
    if ((Test-Path $dest) -and $Force) { Remove-Item $dest -Recurse -Force }
    if (-not (Test-Path (Join-Path $dest '.git'))) {
        Invoke-Native git @('clone', '--recurse-submodules', $Repo, $dest)
    }
    Invoke-Native git @('-C', $dest, 'fetch', '--tags', '--force', 'origin')
    Invoke-Native git @('-C', $dest, 'checkout', '--recurse-submodules', $Commit)
    # A shallow/moved clone could have ended up on a different commit; fail
    # loudly rather than shipping an unpinned build.
    $actual = (& git -C $dest rev-parse HEAD).Trim()
    if ($actual -ne $Commit) { throw "Expected $Name at $Commit but got $actual" }
}

# Downloads and extracts the pinned GLEW release archive, verifying its
# SHA-256. Returns the extracted source root. The archive is cached in
# `$WorkDir` and only re-downloaded when missing.
function Get-GlewSource {
    $dest = Join-Path $WorkDir $GlewTag
    if (Test-Path (Join-Path $dest 'build\cmake\CMakeLists.txt')) { return $dest }
    $zip = Join-Path $WorkDir "$GlewTag.zip"
    if (-not (Test-Path $zip)) {
        Write-Host "Downloading $GlewArchiveUrl" -ForegroundColor DarkGray
        # Invoke-WebRequest's progress bar makes a large download very slow.
        $previous = $ProgressPreference
        $ProgressPreference = 'SilentlyContinue'
        try { Invoke-WebRequest -Uri $GlewArchiveUrl -OutFile $zip }
        finally { $ProgressPreference = $previous }
    }
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $zip).Hash
    if ($actual -ne $GlewArchiveSha256) {
        throw "GLEW archive hash mismatch: expected $GlewArchiveSha256 but got $actual"
    }
    if (Test-Path $dest) { Remove-Item $dest -Recurse -Force }
    Expand-Archive -LiteralPath $zip -DestinationPath $WorkDir -Force
    return $dest
}

function Invoke-CMake([string[]]$Arguments) {
    Invoke-Native cmake $Arguments
}

Import-MsvcEnvironment
if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    throw 'cmake not found on PATH. Install CMake 3.21+ (winget install Kitware.CMake).'
}
$generator = Get-VsGenerator
Write-Host "Using generator: $generator" -ForegroundColor Cyan

New-Item -ItemType Directory -Force $WorkDir, $OutputDir | Out-Null

$glewRoot = Get-GlewSource
Get-PinnedSource 'projectm' $ProjectMRepo $ProjectMCommit

# --- GLEW -------------------------------------------------------------------
# projectM 4.1 includes <GL/glew.h> on Windows and fails to configure without
# it; its CMake prefers the shared GLEW, so the runtime needs glew32.dll. The
# release archive unpacks to <tag>\build\cmake. Its CMake predates CMP0091, so
# `CMAKE_MSVC_RUNTIME_LIBRARY` is ignored: /MT is passed explicitly to match
# projectM's static CRT. CMAKE_POLICY_VERSION_MINIMUM lets its 2.8-era
# `cmake_minimum_required` configure on CMake 4+.
$glewSrc = Join-Path $glewRoot 'build\cmake'
$glewBuild = Join-Path $WorkDir 'glew-build'
$glewInstall = Join-Path $WorkDir 'glew-install'
Invoke-CMake @(
    '-S', $glewSrc
    '-B', $glewBuild
    '-G', $generator, '-A', 'x64'
    "-DCMAKE_INSTALL_PREFIX=$glewInstall"
    '-DBUILD_UTILS=OFF'
    '-DCMAKE_C_FLAGS_RELEASE=/O2 /Ob2 /DNDEBUG /MT'
    '-DCMAKE_POLICY_VERSION_MINIMUM=3.5'
)
Invoke-CMake @('--build', $glewBuild, '--config', 'Release', '--parallel')
Invoke-CMake @('--install', $glewBuild, '--config', 'Release')

# --- projectM ---------------------------------------------------------------
# Static CRT (`/MT`) so the DLLs do not need the VC++ runtime; vendored glm and
# projectm-eval (`ENABLE_SYSTEM_*=OFF`) so the build needs only GLEW. The
# playlist library backs the crate's playlist API.
$projectmSrc = Join-Path $WorkDir 'projectm'
$projectmBuild = Join-Path $WorkDir 'projectm-build'
$projectmInstall = Join-Path $WorkDir 'projectm-install'
Invoke-CMake @(
    '-S', $projectmSrc
    '-B', $projectmBuild
    '-G', $generator, '-A', 'x64'
    "-DCMAKE_INSTALL_PREFIX=$projectmInstall"
    '-DBUILD_SHARED_LIBS=ON'
    '-DENABLE_PLAYLIST=ON'
    '-DENABLE_SDL_UI=OFF'
    '-DENABLE_INSTALL=ON'
    '-DENABLE_SYSTEM_GLM=OFF'
    '-DENABLE_SYSTEM_PROJECTM_EVAL=OFF'
    '-DBUILD_TESTING=OFF'
    '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded'
    "-DCMAKE_PREFIX_PATH=$glewInstall"
)
Invoke-CMake @('--build', $projectmBuild, '--config', 'Release', '--parallel')
Invoke-CMake @('--install', $projectmBuild, '--config', 'Release')

# --- Stage ------------------------------------------------------------------
# Install puts every runtime in <prefix>\bin; copy the three the crate's loader
# looks for from `projectm\` next to the executable.
$artifacts = @(
    @{ Name = 'projectM-4.dll'; Source = Join-Path $projectmInstall 'bin\projectM-4.dll' }
    @{ Name = 'projectM-4-playlist.dll'; Source = Join-Path $projectmInstall 'bin\projectM-4-playlist.dll' }
    @{ Name = 'glew32.dll'; Source = Join-Path $glewInstall 'bin\glew32.dll' }
)
foreach ($artifact in $artifacts) {
    if (-not (Test-Path -LiteralPath $artifact.Source)) {
        throw "Expected $($artifact.Name) at $($artifact.Source); see the CMake output above."
    }
    Copy-Item -LiteralPath $artifact.Source -Destination (Join-Path $OutputDir $artifact.Name) -Force
}

Write-Host "Staged projectM $ProjectMTag + GLEW $GlewTag in $OutputDir" -ForegroundColor Green
Get-ChildItem -LiteralPath $OutputDir -Filter '*.dll' |
    ForEach-Object { Write-Host ("  {0}  {1:N0} KiB" -f $_.Name, ($_.Length / 1KB)) }
