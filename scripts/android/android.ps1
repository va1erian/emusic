<#
.SYNOPSIS
  Agent-facing driver for the emusic Android app.

.DESCRIPTION
  Boots a headless emulator, installs and launches the APK, captures
  screenshots and drives the UI (tap/type/key), so an agent or a developer can
  look at and manipulate the real app without a display.

  Screenshots are pulled as files (never piped through PowerShell, which would
  corrupt the PNG) and can be downscaled to a readable width.

.EXAMPLE
  scripts\android\android.ps1 avd
  scripts\android\android.ps1 start
  scripts\android\android.ps1 install
  scripts\android\android.ps1 launch
  scripts\android\android.ps1 shot -Out shots\main.png -Width 480
  scripts\android\android.ps1 tap -Text Add
  scripts\android\android.ps1 type -Text "http://10.0.2.2:8080"
  scripts\android\android.ps1 dump
  scripts\android\android.ps1 stop

.NOTES
  On a shared desktop, prefer booting with -NoWindow (the default): the
  emulator then never steals focus.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory, Position = 0)]
    [ValidateSet('avd', 'start', 'stop', 'install', 'launch', 'shot', 'dump', 'tap', 'type', 'key', 'wait', 'status')]
    [string]$Command,

    # Android SDK root. Defaults to $env:ANDROID_HOME.
    [string]$Sdk = $env:ANDROID_HOME,

    # AVD name and the device profile/system image used to create it.
    [string]$Name = 'emusic_test',
    [string]$Image = 'system-images;android-36;google_apis;x86_64',
    [string]$Device = 'pixel_6',

    # APK to install; defaults to the debug build output.
    [string]$Apk,

    # Activity to launch.
    [string]$Activity = 'dev.emusic.mobile/.MainActivity',

    # Where to write a screenshot or UI dump.
    [string]$Out,

    # Text or resource-id to match for `tap`, or text for `type`.
    [string]$Text,
    [string]$Id,

    # Key event names for `key` (for example HOME, BACK, ENTER, DEL).
    [string[]]$Key,

    # Downscale screenshots to this pixel width (0 keeps the device resolution).
    [int]$Width = 0,

    # Seconds to wait for boot in `start`/`wait`.
    [int]$WaitSecs = 300,

    # Boot with a visible window (off by default so focus is never stolen).
    [switch]$WithWindow
)

$ErrorActionPreference = 'Stop'

if (-not $Sdk) { throw 'Set ANDROID_HOME or pass -Sdk <path to the Android SDK>.' }
$Sdk = (Resolve-Path -LiteralPath $Sdk).Path

$Adb = Join-Path $Sdk 'platform-tools\adb.exe'
$Emulator = Join-Path $Sdk 'emulator\emulator.exe'
$AvdManager = Join-Path $Sdk 'cmdline-tools\latest\bin\avdmanager.bat'
$SdkManager = Join-Path $Sdk 'cmdline-tools\latest\bin\sdkmanager.bat'
$RemoteShot = '/sdcard/emusic-shot.png'
$RemoteDump = '/sdcard/emusic-ui.xml'

foreach ($tool in @($Adb, $Emulator)) {
    if (-not (Test-Path -LiteralPath $tool)) { throw "Missing Android tool: $tool" }
}

function Invoke-Adb {
    param([string[]]$Arguments)
    # adb writes progress to stderr; `2>&1` turns that into ErrorRecords which
    # would terminate the script under ErrorActionPreference=Stop, so relax it
    # for the call and rely on the exit code instead.
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = & $Adb @Arguments 2>&1
    }
    finally {
        $ErrorActionPreference = $previous
    }
    if ($LASTEXITCODE -ne 0) { throw "adb $($Arguments -join ' ') failed: $output" }
    return $output
}

function Get-DeviceSerial {
    $lines = Invoke-Adb -Arguments @('devices')
    $serial = $lines | Where-Object { $_ -match '^\S+\s+device$' } | Select-Object -First 1
    if (-not $serial) { return $null }
    return ($serial -split '\s+')[0]
}

function Test-EmulatorRunning {
    return [bool](Get-DeviceSerial)
}

function Wait-ForBoot {
    param([int]$TimeoutSecs)
    $deadline = (Get-Date).AddSeconds($TimeoutSecs)
    while ((Get-Date) -lt $deadline) {
        $serial = Get-DeviceSerial
        if ($serial) {
            $boot = (& $Adb -s $serial shell getprop sys.boot_completed 2>$null | Out-String).Trim()
            if ($boot -eq '1') { return $serial }
        }
        Start-Sleep -Seconds 3
    }
    throw "Emulator did not finish booting within $TimeoutSecs s."
}

function Ensure-Avd {
    $installed = & $Emulator -list-avds 2>$null
    if ($installed -contains $Name) { return }

    Write-Host "Installing system image $Image (this can take a while)..."
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $y = ("y`r`n" * 80)
        $y | & $SdkManager $Image 2>&1 | Out-Null

        Write-Host "Creating AVD $Name..."
        'no' | & $AvdManager create avd -n $Name -k $Image -d $Device --force 2>&1 | Out-Null
    }
    finally {
        $ErrorActionPreference = $previous
    }
}

function Start-Emulator {
    if (Test-EmulatorRunning) {
        Write-Host 'An emulator is already running.'
        return
    }
    Ensure-Avd
    $arguments = @(
        '-avd', $Name,
        '-no-audio', '-no-boot-anim', '-no-snapshot',
        '-gpu', 'swiftshader_indirect'
    )
    if (-not $WithWindow) { $arguments += '-no-window' }
    Write-Host "Booting AVD $Name..."
    Start-Process -FilePath $Emulator -ArgumentList $arguments -WindowStyle Hidden
}

function Resize-Image {
    param([string]$Path, [int]$TargetWidth)
    if ($TargetWidth -le 0) { return $Path }
    Add-Type -AssemblyName System.Drawing
    # Save to a sibling first: Image.FromFile locks the source, so writing the
    # resized bitmap back to the same path would fail with a GDI+ error.
    $temporary = "$Path.resized"
    $image = [System.Drawing.Image]::FromFile($Path)
    try {
        $height = [int]($image.Height * $TargetWidth / $image.Width)
        $bitmap = New-Object System.Drawing.Bitmap $TargetWidth, $height
        try {
            $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
            try { $graphics.DrawImage($image, 0, 0, $TargetWidth, $height) }
            finally { $graphics.Dispose() }
            $bitmap.Save($temporary, [System.Drawing.Imaging.ImageFormat]::Png)
        }
        finally { $bitmap.Dispose() }
    }
    finally { $image.Dispose() }
    Move-Item -Force -LiteralPath $temporary -Destination $Path
    return $Path
}

function Save-Screenshot {
    param([string]$Path, [int]$TargetWidth)
    Invoke-Adb -Arguments @('shell', 'screencap', '-p', $RemoteShot) | Out-Null
    $directory = Split-Path -Parent $Path
    if ($directory) { New-Item -ItemType Directory -Force -Path $directory | Out-Null }
    Invoke-Adb -Arguments @('pull', $RemoteShot, $Path) | Out-Null
    return (Resize-Image -Path $Path -TargetWidth $TargetWidth)
}

function Get-UiDocument {
    Invoke-Adb -Arguments @('shell', 'uiautomator', 'dump', $RemoteDump) | Out-Null
    $local = Join-Path ([System.IO.Path]::GetTempPath()) 'emusic-ui.xml'
    Invoke-Adb -Arguments @('pull', $RemoteDump, $local) | Out-Null
    return [xml](Get-Content -LiteralPath $local -Raw)
}

function Find-UiNode {
    param([string]$MatchText, [string]$MatchId)
    $document = Get-UiDocument
    if ($MatchText) {
        $node = $document.SelectSingleNode("//node[@text='$MatchText']")
        if ($node) { return $node }
    }
    if ($MatchId) {
        $node = $document.SelectSingleNode("//node[contains(@resource-id,'$MatchId')]")
        if ($node) { return $node }
    }
    return $null
}

function Get-Center {
    param([string]$Bounds)
    if ($Bounds -notmatch '\[(\d+),(\d+)\]\[(\d+),(\d+)\]') { throw "Unparseable bounds: $Bounds" }
    return @(
        [int](([int]$Matches[1] + [int]$Matches[3]) / 2),
        [int](([int]$Matches[2] + [int]$Matches[4]) / 2)
    )
}

switch ($Command) {
    'avd' {
        Ensure-Avd
        Write-Host "AVD $Name is ready."
    }
    'start' {
        Start-Emulator
        $serial = Wait-ForBoot -TimeoutSecs $WaitSecs
        Write-Host "Ready: $serial"
    }
    'wait' {
        $serial = Wait-ForBoot -TimeoutSecs $WaitSecs
        Write-Host "Ready: $serial"
    }
    'stop' {
        Invoke-Adb -Arguments @('emu', 'kill') | Out-Null
        Write-Host 'Stopped.'
    }
    'install' {
        $path = if ($Apk) { $Apk } else { Join-Path $PSScriptRoot '..\..\android\app\build\outputs\apk\debug\app-debug.apk' }
        if (-not (Test-Path -LiteralPath $path)) { throw "APK not found: $path" }
        Invoke-Adb -Arguments @('install', '-r', $path) | Out-Null
        Write-Host "Installed $path"
    }
    'launch' {
        Invoke-Adb -Arguments @('shell', 'am', 'start', '-S', '-n', $Activity) | Out-Null
        Write-Host "Launched $Activity"
    }
    'shot' {
        if (-not $Out) { throw 'Pass -Out <path.png> for `shot`.' }
        $saved = Save-Screenshot -Path $Out -TargetWidth $Width
        Write-Host $saved
    }
    'dump' {
        $document = Get-UiDocument
        if ($Out) { $document.Save($Out) }
        $document.SelectNodes('//node[@text!=""]') | ForEach-Object {
            "{0}  [{1}]  {2}" -f $_.text, $_.'resource-id', $_.bounds
        }
    }
    'tap' {
        $node = Find-UiNode -MatchText $Text -MatchId $Id
        if (-not $node) { throw "No UI node matched text='$Text' id='$Id'." }
        $point = Get-Center -Bounds $node.bounds
        Invoke-Adb -Arguments @('shell', 'input', 'tap', $point[0], $point[1]) | Out-Null
        Write-Host "Tapped ($($point[0]), $($point[1]))"
    }
    'type' {
        if (-not $Text) { throw 'Pass -Text for `type`.' }
        # `input text` treats spaces as %s; keep other characters simple.
        $escaped = $Text -replace ' ', '%s'
        Invoke-Adb -Arguments @('shell', 'input', 'text', $escaped) | Out-Null
        Write-Host "Typed: $Text"
    }
    'key' {
        if (-not $Key) { throw 'Pass -Key <NAME> for `key`.' }
        foreach ($name in $Key) {
            Invoke-Adb -Arguments @('shell', 'input', 'keyevent', $name) | Out-Null
        }
        Write-Host "Sent: $($Key -join ', ')"
    }
    'status' {
        if (Test-EmulatorRunning) {
            $serial = Get-DeviceSerial
            $activity = (& $Adb -s $serial shell dumpsys window 2>$null | Select-String 'mCurrentFocus' | Select-Object -First 1)
            Write-Host "Device: $serial"
            Write-Host "Focus:  $activity"
        }
        else { Write-Host 'No emulator running.' }
    }
}
