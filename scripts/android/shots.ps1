<#
.SYNOPSIS
  Capture the Android app's reference screenshots from a running emulator.

.DESCRIPTION
  Drives the app through its current views with `android.ps1` and writes a
  small, readable PNG per view under `android/docs/screenshots`. The committed
  set is the reference, mirroring `crates/app/docs/screenshots` on the desktop.

.EXAMPLE
  scripts\android\shots.ps1
#>
[CmdletBinding()]
param(
    [string]$Sdk = $env:ANDROID_HOME,
    [string]$Out,
    [int]$Width = 480
)

$ErrorActionPreference = 'Stop'
$driver = Join-Path $PSScriptRoot 'android.ps1'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if (-not $Out) { $Out = Join-Path $root 'android\docs\screenshots' }
New-Item -ItemType Directory -Force -Path $Out | Out-Null

function Shot {
    param([string]$Name)
    & $driver shot -Sdk $Sdk -Out (Join-Path $Out $Name) -Width $Width | Out-Null
    Write-Host "wrote $Name"
}

& $driver start -Sdk $Sdk
& $driver install -Sdk $Sdk
& $driver launch -Sdk $Sdk
Start-Sleep -Seconds 3

# `launch` force-stops first, so the app opens on a clean server list.
Shot 'servers.png'

# Add-server form.
& $driver tap -Sdk $Sdk -Text Add
Start-Sleep -Seconds 1
Shot 'add-server.png'

Write-Host "Reference screenshots in $Out"
