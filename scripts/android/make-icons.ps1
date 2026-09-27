<#
.SYNOPSIS
  Regenerate the Android launcher icons from the Windows app icon.

.DESCRIPTION
  Extracts the 256x256 PNG frame from `assets/ico/emusic-app.ico` and writes
  the Android `mipmap-*` launcher assets (legacy square, round, and the
  adaptive-icon foreground). The adaptive-icon XML and background colour live
  under `android/app/src/main/res` and are committed as-is.

  Run from anywhere:  scripts\android\make-icons.ps1
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$icoPath = Join-Path $root 'assets\ico\emusic-app.ico'
$res = Join-Path $root 'android\app\src\main\res'
$scratch = Join-Path ([System.IO.Path]::GetTempPath()) 'emusic-256.png'

# --- Extract the largest (256x256) PNG frame from the .ico ------------------
$bytes = [IO.File]::ReadAllBytes($icoPath)
$count = [BitConverter]::ToUInt16($bytes, 4)
$best = $null
for ($i = 0; $i -lt $count; $i++) {
    $off = 6 + $i * 16
    $w = [int]$bytes[$off]; if ($w -eq 0) { $w = 256 }
    $h = [int]$bytes[$off + 1]; if ($h -eq 0) { $h = 256 }
    if ($best -eq $null -or $w -gt $best.w) {
        $best = @{
            w    = $w
            h    = $h
            size = [BitConverter]::ToUInt32($bytes, $off + 8)
            off  = [BitConverter]::ToUInt32($bytes, $off + 12)
        }
    }
}
if (-not ($bytes[$best.off] -eq 0x89 -and $bytes[$best.off + 1] -eq 0x50)) {
    throw 'The largest icon frame is not a PNG; update this script.'
}
$slice = New-Object byte[] $best.size
[Array]::Copy($bytes, $best.off, $slice, 0, $best.size)
[IO.File]::WriteAllBytes($scratch, $slice)

$src = [System.Drawing.Image]::FromFile($scratch)

function Save-Square($img, $size, $path) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)
    $g.DrawImage($img, 0, 0, $size, $size)
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}

function Save-Round($img, $size, $path) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.Clear([System.Drawing.Color]::Transparent)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $clip = New-Object System.Drawing.Drawing2D.GraphicsPath
    $clip.AddEllipse(0, 0, $size, $size)
    $g.SetClip($clip)
    $g.DrawImage($img, 0, 0, $size, $size)
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose(); $clip.Dispose()
}

function Save-Foreground($img, $canvas, $path) {
    $bmp = New-Object System.Drawing.Bitmap $canvas, $canvas
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.Clear([System.Drawing.Color]::Transparent)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    # Keep the art inside the adaptive-icon safe zone (66 of 108 dp).
    $art = [int]($canvas * 0.62)
    $offset = [int](($canvas - $art) / 2)
    $g.DrawImage($img, $offset, $offset, $art, $art)
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}

$densities = @{ 'mdpi' = 48; 'hdpi' = 72; 'xhdpi' = 96; 'xxhdpi' = 144; 'xxxhdpi' = 192 }
foreach ($density in $densities.Keys) {
    $size = $densities[$density]
    $dir = Join-Path $res "mipmap-$density"
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    Save-Square $src $size (Join-Path $dir 'ic_launcher.png')
    Save-Round $src $size (Join-Path $dir 'ic_launcher_round.png')
    Save-Foreground $src ([int]($size * 108 / 48)) (Join-Path $dir 'ic_launcher_foreground.png')
}
$src.Dispose()
Remove-Item -LiteralPath $scratch -ErrorAction SilentlyContinue
Write-Host "Wrote launcher icons to $res"
