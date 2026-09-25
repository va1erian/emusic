<#
.SYNOPSIS
    Runs emusic's test binaries (or any Windows GUI executable) inside Windows
    Sandbox, so windows, focus changes and synthetic input stay off your desktop.

.DESCRIPTION
    1. Builds on the host (`cargo test --no-run`, or `cargo build`) into a
       separate target dir with a static CRT, because the sandbox image has no
       Visual C++ runtime.
    2. Stages the executables, each crate's runtime test data, the BASS DLLs
       when available, and a runner script under `<target>\sandbox-stage`.
    3. Launches a disposable, network-less Windows Sandbox that maps the stage
       folder, runs every executable with the right working directory, writes
       logs to `out\` and shuts down.
    4. Prints the logs and exits non-zero if anything failed.

    Nothing is installed in the sandbox and nothing survives it.

    Unlike a plain `cargo test`, this runs each test binary from a staged copy
    of its crate's `tests\` folder, so tests that read runtime data find it.
    Pass `-CargoArgs '--features','emusic/shot'` to match CI's feature set, and
    `-BassDir` to run the BASS-backed tests for real.

.EXAMPLE
    # Every test in the workspace
    scripts\sandbox\run.ps1 -BassDir C:\BASS\x64

.EXAMPLE
    # One integration test binary, with the CI feature set and a filter
    scripts\sandbox\run.ps1 -CargoArgs '--features','emusic/shot' -TestArgs 'music'

.EXAMPLE
    # The real app: launch it, capture the sandbox desktop, stop it
    scripts\sandbox\run.ps1 -Build -CargoArgs '--bin','emusic' -Screenshot

.EXAMPLE
    # An executable you already built, with BASS staged next to it
    scripts\sandbox\run.ps1 -Exe .\target\debug\emusic.exe -BassDir C:\BASS\x64 -Screenshot
#>
[CmdletBinding()]
param(
    # Cargo.toml of the crate or workspace to build. Defaults to the repo root.
    [string]$ManifestPath,
    # Extra cargo arguments, e.g. '--features','emusic/shot' or '-p','emusic'.
    [string[]]$CargoArgs = @(),
    # Arguments passed to every executable (libtest filters, --ignored, ...).
    [string[]]$TestArgs = @(),
    # `cargo build` and run the produced binaries instead of `cargo test`.
    [switch]$Build,
    # Pre-built executables to run; skips cargo entirely.
    [string[]]$Exe = @(),
    # Real BASS install: `bass*.dll` are staged and `EMUSIC_BASS_DIR` points at
    # them, so the BASS-backed tests run instead of skipping. Defaults to
    # `$env:EMUSIC_BASS_DIR`.
    [string]$BassDir = $env:EMUSIC_BASS_DIR,
    # Environment variables set inside the sandbox before the runs.
    [hashtable]$Env = @{},
    # Minutes to wait for the sandbox before giving up.
    [int]$TimeoutMinutes = 30,
    [int]$MemoryMB = 4096,
    # Turn the virtual GPU on. Off by default: on some GPU drivers it makes the
    # whole sandbox VM die (0x80370106) as soon as a GPU test starts, and the
    # app's Direct2D/Mica render fine on WARP, the software rasteriser
    # (verified). Enable it only when you need a hardware GPU path.
    [switch]$EnableVgpu,
    # Override the build/stage root. Defaults to `<manifest dir>\target\sandbox`.
    [string]$TargetDir,
    # Launch each executable, wait -ScreenshotDelayMs, save a PNG of the
    # sandbox desktop to out\<name>.png, then stop the executable if it is
    # still running. For looking at an app rather than running tests.
    [switch]$Screenshot,
    [int]$ScreenshotDelayMs = 3000,
    # Leave the sandbox open after the runs, to inspect it.
    [switch]$Keep
)
$ErrorActionPreference = 'Stop'

# `$PSScriptRoot` is not available while parameter defaults are evaluated on
# Windows PowerShell 5.1, so resolve the default here instead.
if (-not $ManifestPath) {
    $ManifestPath = Join-Path $PSScriptRoot '..\..\Cargo.toml'
}

$sandboxExe = Join-Path $env:windir 'System32\WindowsSandbox.exe'
if (-not (Test-Path $sandboxExe)) {
    throw "Windows Sandbox is not enabled. Run once as admin, then reboot:`n" +
          "  Enable-WindowsOptionalFeature -Online -FeatureName Containers-DisposableClientVM"
}
if (Get-Process -Name WindowsSandboxRemoteSession, WindowsSandboxClient -ErrorAction SilentlyContinue) {
    throw 'A Windows Sandbox is already running; only one can run at a time. Close it first.'
}

$manifest = (Resolve-Path $ManifestPath).Path
if (-not $TargetDir) {
    $TargetDir = Join-Path (Split-Path $manifest) 'target\sandbox'
}
$targetDir = $TargetDir
$stage = Join-Path $targetDir 'sandbox-stage'
$bin = Join-Path $stage 'bin'
$out = Join-Path $stage 'out'
$pkg = Join-Path $stage 'pkg'
$bass = Join-Path $stage 'bass'
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory $bin, $out, $pkg | Out-Null

# --- Build on the host ------------------------------------------------------
# Each entry becomes one run in the sandbox: a staged file name, the host
# executable, and the crate root that must be the working directory when the
# crate has a `tests\` folder.
$artifacts = @()
if ($Exe.Count -eq 0) {
    $env:CARGO_TARGET_DIR = $targetDir
    # The sandbox has no vcruntime140.dll; a static CRT makes the binaries self-contained.
    $env:RUSTFLAGS = "$env:RUSTFLAGS -C target-feature=+crt-static".Trim()
    # @() keeps a one-element array from collapsing to a string, which `@cmd`
    # would then splat character by character.
    $cmd = @(if ($Build) { 'build' } else { 'test'; '--no-run' })
    # Native tools write progress to stderr. A caller that pipes this script
    # makes PowerShell 5.1 turn that into a terminating error under `Stop`, so
    # relax the preference for the call and let cargo's progress through.
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $cargoOut = & cargo @cmd --manifest-path $manifest --message-format=json-render-diagnostics @CargoArgs
    $cargoCode = $LASTEXITCODE
    $ErrorActionPreference = $prevEap
    if ($cargoCode -ne 0) { throw "cargo $($cmd -join ' ') failed" }
    $artifacts = @($cargoOut | ForEach-Object {
        if ($_ -notmatch '^\{') { return }
        $msg = $_ | ConvertFrom-Json
        if ($msg.reason -ne 'compiler-artifact' -or -not $msg.executable) { return }
        # Build scripts and proc-macros are artifacts too, but not runnable tests.
        if ($msg.target.kind -contains 'custom-build') { return }
        if (-not $Build -and -not $msg.profile.test) { return }
        [pscustomobject]@{
            Executable   = $msg.executable
            ManifestPath = $msg.manifest_path
            TargetName   = $msg.target.name
        }
    } | Sort-Object Executable -Unique)
} else {
    $artifacts = @($Exe | ForEach-Object {
        [pscustomobject]@{
            Executable   = $_
            ManifestPath = $null
            TargetName   = [IO.Path]::GetFileNameWithoutExtension($_)
        }
    })
}
if ($artifacts.Count -eq 0) { throw 'Nothing to run.' }

# --- Stage executables, per-crate test data and BASS ------------------------
$entries = @()
$usedNames = @{}
$takenNames = @{}
$stagedPackages = @{}
foreach ($artifact in $artifacts) {
    # Cargo names an integration test binary after its target, so two crates
    # can both produce `smoke.exe`. Stage each under a unique leaf name.
    $base = $artifact.TargetName
    $usedNames[$base] = [int]$usedNames[$base] + 1
    $n = $usedNames[$base]
    $name = if ($n -eq 1) { $base } else { "$base-$n" }
    while ($takenNames.ContainsKey($name)) {
        $n++
        $name = "$base-$n"
    }
    $takenNames[$name] = $true

    $dest = Join-Path $bin "$name.exe"
    Copy-Item -LiteralPath (Resolve-Path $artifact.Executable).Path -Destination $dest

    $workDir = $null
    if ($artifact.ManifestPath) {
        $pkgDir = Split-Path $artifact.ManifestPath
        $pkgName = Split-Path $pkgDir -Leaf
        $pkgStage = Join-Path $pkg $pkgName
        if (-not $stagedPackages.ContainsKey($pkgStage)) {
            # The destination must exist first, or `Copy-Item` renames the
            # source folder to the destination instead of nesting it.
            New-Item -ItemType Directory -Force $pkgStage | Out-Null
            # These are copy-if-present, so a test that reads something
            # relative to the crate root (which cargo makes the working
            # directory) finds it.
            foreach ($sub in 'tests', 'Cargo.toml', 'kittest.toml', 'assets') {
                $from = Join-Path $pkgDir $sub
                if (Test-Path -LiteralPath $from) {
                    Copy-Item -LiteralPath $from -Destination $pkgStage -Recurse -Force
                }
            }
            $stagedPackages[$pkgStage] = $true
        }
        if (Test-Path -LiteralPath $pkgStage) {
            $workDir = "C:\stage\pkg\$pkgName"
        }
    }
    $entries += [pscustomobject]@{
        name    = $name
        path    = "C:\stage\bin\$name.exe"
        workdir = $workDir
    }
}

$runEnv = @{} + $Env
if ($BassDir) {
    if (-not (Test-Path -LiteralPath $BassDir)) { throw "BASS dir not found: $BassDir" }
    $dlls = @(Get-ChildItem -LiteralPath $BassDir -File -Filter '*.dll' |
        Where-Object { $_.Name -match '^bass' })
    if ($dlls.Count -eq 0) {
        Write-Warning "no bass*.dll found in $BassDir; BASS tests will skip"
    } else {
        New-Item -ItemType Directory $bass | Out-Null
        $dlls | Copy-Item -Destination $bass
        if (-not $runEnv.ContainsKey('EMUSIC_BASS_DIR')) {
            $runEnv['EMUSIC_BASS_DIR'] = 'C:\stage\bass'
        }
        Write-Host "Staged $($dlls.Count) BASS DLL(s) from $BassDir"
    }
} else {
    Write-Warning 'No -BassDir or $env:EMUSIC_BASS_DIR; BASS-backed tests will skip. Pass -BassDir to run them.'
}

# --- Runner executed inside the sandbox ------------------------------------
$envLines = ($runEnv.GetEnumerator() | ForEach-Object {
    "`$env:$($_.Key) = '$($_.Value -replace "'", "''")'"
}) -join "`n"

$runnerTemplate = @'
$ErrorActionPreference = 'Continue'
$env:RUST_BACKTRACE = '1'
__ENVLINES__
$failed = 0
$screenshot = __SCREENSHOT__
$delayMs = __DELAY__
# Assign first: on Windows PowerShell, `@(ConvertFrom-Json ...)` wraps the
# whole array as one element instead of enumerating it.
$exes = ConvertFrom-Json -InputObject ([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('__EXE64__')))
$argList = ConvertFrom-Json -InputObject ([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('__ARG64__')))
Add-Type -AssemblyName System.Drawing, System.Windows.Forms
function Save-Desktop($path) {
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}
Write-Host "emusic sandbox: $($exes.Count) executable(s) to run" -ForegroundColor Cyan
foreach ($e in $exes) {
    $log = "C:\stage\out\$($e.name).log"
    $wd = if ($e.workdir -and (Test-Path $e.workdir)) { $e.workdir } else { 'C:\stage\bin' }
    Write-Host ">>> $($e.name)" -ForegroundColor Cyan
    Set-Location $wd
    if ($screenshot) {
        $start = @{ FilePath = $e.path; WorkingDirectory = $wd; PassThru = $true
                    RedirectStandardOutput = $log; RedirectStandardError = "$log.err" }
        if ($argList.Count) { $start.ArgumentList = $argList }
        $p = Start-Process @start
        Start-Sleep -Milliseconds $delayMs
        Save-Desktop "C:\stage\out\$($e.name).png"
        # Still running after the capture is the expected case for an app.
        $killed = -not $p.HasExited
        if ($killed) { $p | Stop-Process -Force; $p.WaitForExit() }
        $global:LASTEXITCODE = if ($killed) { 0 } else { $p.ExitCode }
    } else {
        # Start-Process rather than `&`: it captures native stdout and stderr
        # into separate files without PowerShell reformatting a tool's stderr
        # (the shot tools log "wrote ...png" there) as an error record.
        $start = @{ FilePath = $e.path; WorkingDirectory = $wd; Wait = $true; PassThru = $true
                    NoNewWindow = $true
                    RedirectStandardOutput = $log; RedirectStandardError = "$log.err" }
        if ($argList.Count) { $start.ArgumentList = $argList }
        $p = Start-Process @start
        $global:LASTEXITCODE = $p.ExitCode
    }
    "$($e.name) exit=$LASTEXITCODE" | Add-Content C:\stage\out\summary.txt
    if ($LASTEXITCODE -ne 0) { $failed++ }
}
Set-Content C:\stage\out\done.txt $failed
'@

# Base64 keeps the JSON out of PowerShell's parser, which would treat a
# literal `[...]` as a type name. `[]` still encodes to a non-empty payload.
function ConvertTo-Base64([string]$text) {
    [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($text))
}
$exe64 = ConvertTo-Base64 (ConvertTo-Json -InputObject @($entries) -Depth 3 -Compress)
$arg64 = ConvertTo-Base64 (ConvertTo-Json -InputObject @($TestArgs) -Compress)
$runner = $runnerTemplate.
    Replace('__ENVLINES__', $envLines).
    Replace('__SCREENSHOT__', $(if ($Screenshot) { '$true' } else { '$false' })).
    Replace('__DELAY__', "$ScreenshotDelayMs").
    Replace('__EXE64__', $exe64).
    Replace('__ARG64__', $arg64)
$runnerPath = Join-Path $stage 'runner.ps1'
Set-Content $runnerPath $runner -Encoding UTF8
# The sandbox desktop is expensive to start and opaque to debug: a runner
# that does not parse would make it sit there doing nothing until the timeout.
$tokens = $null
$parseErrors = $null
[void][System.Management.Automation.Language.Parser]::ParseFile($runnerPath, [ref]$tokens, [ref]$parseErrors)
if ($parseErrors) {
    throw "generated runner.ps1 has syntax errors: $($parseErrors.Message -join '; ')"
}

# --- Sandbox configuration --------------------------------------------------
$wsb = Join-Path $targetDir 'tests.wsb'
$vgpu = if ($EnableVgpu) { 'Enable' } else { 'Disable' }
@"
<Configuration>
  <Networking>Disable</Networking>
  <vGPU>$vgpu</vGPU>
  <ClipboardRedirection>Disable</ClipboardRedirection>
  <AudioInput>Disable</AudioInput>
  <VideoInput>Disable</VideoInput>
  <MemoryInMB>$MemoryMB</MemoryInMB>
  <MappedFolders>
    <MappedFolder>
      <HostFolder>$stage</HostFolder>
      <SandboxFolder>C:\stage</SandboxFolder>
      <ReadOnly>false</ReadOnly>
    </MappedFolder>
  </MappedFolders>
  <LogonCommand>
    <Command>powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\stage\runner.ps1</Command>
  </LogonCommand>
</Configuration>
"@ | Set-Content $wsb -Encoding UTF8

Write-Host "Running $($entries.Count) executable(s) in Windows Sandbox..."
Start-Process $sandboxExe -ArgumentList "`"$wsb`""

$done = Join-Path $out 'done.txt'
$deadline = (Get-Date).AddMinutes($TimeoutMinutes)
$sawVm = $false
while (-not (Test-Path $done) -and (Get-Date) -lt $deadline) {
    if (Get-Process -Name WindowsSandboxRemoteSession -ErrorAction SilentlyContinue) {
        $sawVm = $true
    } elseif ($sawVm) {
        # The VM died mid-run, usually as 0x80370106 from a GPU/hypervisor
        # interaction. The vGPU is off by default; if it was explicitly asked
        # for, say so, because dropping it is the first thing to try.
        $hint = if ($EnableVgpu) { '; retry without -EnableVgpu' } else { '' }
        throw "Windows Sandbox terminated before the tests finished (0x80370106?)$hint."
    }
    Start-Sleep -Seconds 2
}

function Stop-Sandbox {
    # Close the sandbox window first so the VM shuts down cleanly; a hard kill
    # can surface the "Windows Sandbox terminated unexpectedly" dialog.
    $client = Get-Process -Name WindowsSandboxClient, WindowsSandbox -ErrorAction SilentlyContinue
    foreach ($p in $client) { [void]$p.CloseMainWindow() }
    $stopDeadline = (Get-Date).AddSeconds(20)
    while ((Get-Date) -lt $stopDeadline -and
           (Get-Process -Name WindowsSandbox, WindowsSandboxClient, WindowsSandboxRemoteSession `
               -ErrorAction SilentlyContinue)) {
        Start-Sleep -Milliseconds 500
    }
    Get-Process -Name WindowsSandbox, WindowsSandboxClient, WindowsSandboxRemoteSession `
        -ErrorAction SilentlyContinue | Stop-Process -Force
}
if (-not $Keep) { Stop-Sandbox }

Get-ChildItem $out -Filter *.log* | Where-Object { $_.Length -gt 0 } | ForEach-Object {
    Write-Host "===== $($_.BaseName) =====" -ForegroundColor Cyan
    Get-Content $_.FullName
}
if (-not (Test-Path $done)) {
    throw "Timed out after $TimeoutMinutes minutes; logs are in $out"
}
Get-Content (Join-Path $out 'summary.txt')
$failed = [int](Get-Content $done)
Get-ChildItem $out -Recurse -Filter *.png | ForEach-Object { Write-Host "Screenshot: $($_.FullName)" }
Write-Host "Logs: $out"
exit $(if ($failed -gt 0) { 1 } else { 0 })
