<#
.SYNOPSIS
    Fetches the pinned projectM preset packs for the emusic installers (#299).

.DESCRIPTION
    Downloads each pack from the projectM preset repositories at the commit
    pinned below, stages them next to each other, packs the optional
    (download-only) packs into one .7z each and refreshes the archive URLs and
    SHA-256 hashes in installer\presets-archives.iss.

    Nothing is committed to the repository: the packs live under target\presets
    and are fetched again on demand. installer\presets.iss (shared by the main
    installer and the emusic-presets-setup.exe helper) reads the generated
    presets-archives.iss and downloads the large packs at install time.

    Staging layout (mirrors the install layout):

      <OutDir>\milkdrop-original\...                        bundled base presets
      <OutDir>\textures\...                                 bundled textures
      <OutDir>\visualizations\presets\cream-of-the-crop\... optional download
      <OutDir>\visualizations\presets\en-d\...              optional download
      <OutDir>\visualizations\presets\projectm-classic\...  optional download
      <OutDir>\archives\<pack>.7z                           one archive per pack

    The bundled folders match the installer's [Files] entries; each archive
    stores visualizations\presets\<pack>\..., so the installer extracts it
    straight into {app}.

.PARAMETER Pack
    Pack names to fetch (default: all). Use the folder names from the table,
    e.g. 'milkdrop-original', 'textures', 'cream-of-the-crop', 'en-d',
    'projectm-classic'. 'all' selects every pack.

.PARAMETER OutDir
    Staging directory. Defaults to <repo>\target\presets.

.PARAMETER ArchiveDir
    Directory for the generated .7z files. Defaults to <OutDir>\archives.

.PARAMETER ReleaseTag
    GitHub release tag the archives are (or will be) published under. Defaults
    to viz-presets-<yyyyMMdd>.

.PARAMETER Repo
    GitHub repository that hosts the release assets. Defaults to
    va1erian/emusic.

.PARAMETER SkipDownload
    Only rebuild the archives and presets-archives.iss from what is already
    staged; never touch the network.

.PARAMETER NoArchive
    Do not build the .7z archives (useful to only stage the bundled base set).

.PARAMETER NoUpdate
    Do not regenerate installer\presets-archives.iss.

.PARAMETER Force
    Re-download and re-extract even when the staged folder already exists.

.EXAMPLE
    scripts\fetch-presets.ps1
    # Fetch everything and refresh the hashes with today's release tag.

.EXAMPLE
    scripts\fetch-presets.ps1 -Pack milkdrop-original,textures
    # Only the bundled base set + textures, no archives.

.EXAMPLE
    scripts\fetch-presets.ps1 -ReleaseTag viz-presets-20260101 -Force
#>
[CmdletBinding()]
param(
    [string[]]$Pack = @('all'),
    [string]$OutDir,
    [string]$ArchiveDir,
    [string]$ReleaseTag = "viz-presets-$((Get-Date).ToString('yyyyMMdd'))",
    [string]$Repo = 'va1erian/emusic',
    [switch]$SkipDownload,
    [switch]$NoArchive,
    [switch]$NoUpdate,
    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Pinned source repositories (#299). `Dest` is the path under the staging root
# (and, for the archives, under the install root). `Source` is the folder to
# copy out of the repository, or '' for the repository root.
$Packs = @(
    @{
        Name = 'milkdrop-original'
        Repo = 'presets-milkdrop-original'
        Commit = 'e03b83e3338d8f1ed6cbcf908c719f249ef24288'
        Source = 'Milkdrop-Original'
        Dest = 'milkdrop-original'
        Title = 'milkdrop-original (552 presets, 4.6 MB)'
        Bundled = $true
        Download = $false
    }
    @{
        Name = 'textures'
        Repo = 'presets-milkdrop-texture-pack'
        Commit = '6368812f27bc747b517218fbf89d21d59afce4d9'
        Source = 'textures'
        Dest = 'textures'
        Title = 'milkdrop-texture-pack (67 textures, 3.5 MB)'
        Bundled = $true
        Download = $false
    }
    @{
        Name = 'cream-of-the-crop'
        Repo = 'presets-cream-of-the-crop'
        Commit = '0180df21f5e0bd39b9060cc5de420ed2f1f9e509'
        Source = ''
        Dest = 'visualizations\presets\cream-of-the-crop'
        Title = 'cream-of-the-crop (9,795 presets, 111 MB)'
        Bundled = $false
        Download = $true
    }
    @{
        Name = 'en-d'
        Repo = 'presets-en-d'
        Commit = 'fff71ea81223109f3558351667eef851f2781c96'
        Source = ''
        Dest = 'visualizations\presets\en-d'
        Title = 'en-d (40 presets, 2.8 MB)'
        Bundled = $false
        Download = $true
    }
    @{
        Name = 'projectm-classic'
        Repo = 'presets-projectm-classic'
        Commit = '14a6244a7d32eb7e114e1a92d1cb93358cdcc54a'
        Source = ''
        Dest = 'visualizations\presets\projectm-classic'
        Title = 'projectm-classic (4,188 presets, 24.6 MB)'
        Bundled = $false
        Download = $true
    }
)

# The download-only packs, in the order installer\presets.iss indexes them.
$DownloadPacks = @('cream-of-the-crop', 'en-d', 'projectm-classic')

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (-not $OutDir) { $OutDir = Join-Path $RepoRoot 'target\presets' }
if (-not $ArchiveDir) { $ArchiveDir = Join-Path $OutDir 'archives' }
$CacheDir = Join-Path $OutDir 'cache'
$IssPath = Join-Path $RepoRoot 'installer\presets-archives.iss'

function Get-Pack {
    param([string]$Name)
    $match = $Packs | Where-Object { $_.Name -eq $Name }
    if (-not $match) { throw "Unknown preset pack '$Name'. Known packs: $($Packs.Name -join ', ')." }
    return $match
}

function Resolve-PackSelection {
    param([string[]]$Names)
    if ($Names.Count -eq 1 -and $Names[0] -eq 'all') { return @($Packs) }
    return @($Names | ForEach-Object { Get-Pack $_ })
}

function Get-SevenZip {
    $candidates = New-Object System.Collections.Generic.List[string]
    foreach ($name in '7z', '7za') {
        $cmd = Get-Command $name -ErrorAction SilentlyContinue
        if ($cmd) { $candidates.Add($cmd.Source) }
    }
    foreach ($path in "$env:ProgramFiles\7-Zip\7z.exe", "${env:ProgramFiles(x86)}\7-Zip\7z.exe") {
        $candidates.Add($path)
    }
    $found = $candidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
    if (-not $found) {
        throw '7-Zip (7z.exe) is required to build the preset archives; install it or add it to PATH.'
    }
    return $found
}

function Get-Tar {
    # Prefer Windows' bundled bsdtar: Git's MSYS tar mangles the "C:" drive
    # prefix in absolute paths.
    $systemTar = Join-Path $env:SystemRoot 'System32\tar.exe'
    if ($env:SystemRoot -and (Test-Path $systemTar)) { return $systemTar }
    $cmd = Get-Command tar -ErrorAction SilentlyContinue
    if (-not $cmd) { throw 'tar is required on PATH to extract the preset archives.' }
    return $cmd.Source
}

function Invoke-PackDownload {
    param([hashtable]$Pack)
    $archive = Join-Path $CacheDir "$($Pack.Repo)-$($Pack.Commit).tar.gz"
    if ((Test-Path $archive) -and -not $Force) { return $archive }
    $url = "https://github.com/projectM-visualizer/$($Pack.Repo)/archive/$($Pack.Commit).tar.gz"
    Write-Host "Downloading $($Pack.Name) from $($Pack.Repo)@$($Pack.Commit.Substring(0, 8))..."
    Invoke-WebRequest -Uri $url -OutFile $archive -UseBasicParsing
    return $archive
}

function Expand-Pack {
    param([hashtable]$Pack)
    $dest = Join-Path $OutDir $Pack.Dest
    if ((Test-Path $dest) -and -not $Force) {
        Write-Host "Already staged: $($Pack.Name)"
        return
    }
    $archive = Invoke-PackDownload $Pack
    $extractRoot = Join-Path $OutDir 'cache\extract'
    $sourceRoot = Join-Path $extractRoot "$($Pack.Repo)-$($Pack.Commit)"
    if (Test-Path $sourceRoot) { Remove-Item -Recurse -Force $sourceRoot }
    New-Item -ItemType Directory -Force -Path $extractRoot | Out-Null
    & (Get-Tar) -xzf $archive -C $extractRoot
    if ($LASTEXITCODE -ne 0) { throw "tar failed to extract $archive." }

    $source = if ($Pack.Source) { Join-Path $sourceRoot $Pack.Source } else { $sourceRoot }
    if (-not (Test-Path $source)) { throw "Pack '$($Pack.Name)' has no '$($Pack.Source)' folder at the pinned commit." }
    if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
    New-Item -ItemType Directory -Force -Path $dest | Out-Null
    Copy-Item -Path (Join-Path $source '*') -Destination $dest -Recurse -Force
    Remove-Item -Recurse -Force $sourceRoot
    Write-Host "Staged $($Pack.Name) -> $dest"
}

function New-PackArchive {
    param([hashtable]$Pack, [string]$SevenZip)
    $dest = Join-Path $OutDir $Pack.Dest
    if (-not (Test-Path $dest)) { throw "Cannot archive $($Pack.Name): $dest is missing." }
    $output = Join-Path $ArchiveDir "$($Pack.Name).7z"
    New-Item -ItemType Directory -Force -Path $ArchiveDir | Out-Null
    if (Test-Path $output) { Remove-Item -Force $output }
    # Archive the staged folder relative to the root so it keeps the
    # visualizations\presets\<pack>\ layout the installer extracts into {app}.
    Push-Location $OutDir
    try {
        & $SevenZip a -t7z -mx=9 -y $output $Pack.Dest | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "7-Zip failed to create $output." }
    } finally {
        Pop-Location
    }
    Write-Host "Packed $($Pack.Name) -> $output"
    return $output
}

function Write-PresetArchivesIss {
    param([string]$Tag)
    $urls = New-Object System.Collections.Generic.List[string]
    $hashes = New-Object System.Collections.Generic.List[string]
    foreach ($name in $DownloadPacks) {
        $archive = Join-Path $ArchiveDir "$name.7z"
        if (Test-Path $archive) {
            $hash = (Get-FileHash -Algorithm SHA256 -Path $archive).Hash.ToLowerInvariant()
            $url = "https://github.com/$Repo/releases/download/$Tag/$name.7z"
        } else {
            $hash = ''
            $url = ''
        }
        $urls.Add($url)
        $hashes.Add($hash)
    }

    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add('{ Generated by scripts\fetch-presets.ps1 (#299). Do not edit by hand. }')
    $lines.Add('{ }')
    $lines.Add('{ Holds the pinned release tag, download URLs and SHA-256 hashes of the }')
    $lines.Add('{ optional projectM preset packs, indexed like the table in presets.iss. }')
    $lines.Add("const")
    $lines.Add("  PresetReleaseTag = '$Tag';")
    $lines.Add('')
    $lines.Add('function PresetArchiveUrlOf(Index: Integer): String;')
    $lines.Add('begin')
    $lines.Add('  case Index of')
    for ($i = 0; $i -lt $DownloadPacks.Count; $i++) {
        $lines.Add("    ${i}: Result := '$($urls[$i])';")
    }
    $lines.Add('  else')
    $lines.Add("    Result := '';")
    $lines.Add('  end;')
    $lines.Add('end;')
    $lines.Add('')
    $lines.Add('function PresetArchiveSha256Of(Index: Integer): String;')
    $lines.Add('begin')
    $lines.Add('  case Index of')
    for ($i = 0; $i -lt $DownloadPacks.Count; $i++) {
        $lines.Add("    ${i}: Result := '$($hashes[$i])';")
    }
    $lines.Add('  else')
    $lines.Add("    Result := '';")
    $lines.Add('  end;')
    $lines.Add('end;')
    $lines.Add('')
    Set-Content -Path $IssPath -Value $lines -Encoding ASCII
    Write-Host "Updated $IssPath"
}

# --- main ------------------------------------------------------------------

$selected = @(Resolve-PackSelection -Names $Pack)
New-Item -ItemType Directory -Force -Path $OutDir, $CacheDir | Out-Null

if (-not $SkipDownload) {
    foreach ($presetPack in $selected) { Expand-Pack $presetPack }
}

if (-not $NoArchive) {
    $sevenZip = Get-SevenZip
    foreach ($presetPack in $selected | Where-Object { $_.Download }) {
        New-PackArchive -Pack $presetPack -SevenZip $sevenZip | Out-Null
    }
}

if (-not $NoUpdate) {
    Write-PresetArchivesIss -Tag $ReleaseTag
}

Write-Host ''
Write-Host "Done. Packs staged under $OutDir; archives under $ArchiveDir."
Write-Host "Bundled:  $($Packs | Where-Object { $_.Bundled } | ForEach-Object { $_.Name })"

