<#
.SYNOPSIS
  Drive the app through UI Automation (for agents and tests).

.DESCRIPTION
  Dot-source this file, then use the functions:

    . .\scripts\win32-uia.ps1
    $app = Start-Emusic                       # launches `emusic --mock`
    Show-UiaTree $app -MaxDepth 2             # print the element tree
    Invoke-Uia $app 'Settings'                # menu item / button: Invoke, else Select
    Invoke-Uia $app 'Playback'                # a tab
    Click-Uia $app 'Smooth'                   # REAL mouse click at the element's centre
    Send-Key 9                                # real key press (VK_TAB)
    Test-Responding $app                      # $false if the UI thread is stuck
    Stop-Emusic $app

  Use Click-Uia / Send-Key (real input through the message loop) for anything
  that depends on mouse or keyboard handling; Invoke-Uia calls the control's
  action directly and skips the message pump, so it cannot reproduce input bugs.

  Do not raise or move other windows: Click-Uia moves the pointer, so prefer it
  on a machine you are not using, and never leave the process running.
#>

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type @"
using System; using System.Runtime.InteropServices;
public class UiaInput {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, int e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte sc, uint fl, int ex);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
"@
[UiaInput]::SetProcessDPIAware() | Out-Null

$script:AE = [System.Windows.Automation.AutomationElement]
$script:Walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker

function Start-Emusic {
  param([string]$Exe = "$PSScriptRoot\..\target\debug\emusic.exe", [string[]]$Arguments = @('--mock'), [int]$WaitSeconds = 4)
  $process = Start-Process (Resolve-Path $Exe) -ArgumentList $Arguments -PassThru
  Start-Sleep $WaitSeconds
  $cond = New-Object System.Windows.Automation.PropertyCondition($AE::ProcessIdProperty, $process.Id)
  [pscustomobject]@{ Process = $process; Window = $AE::RootElement.FindFirst('Children', $cond) }
}

function Stop-Emusic($app) { Stop-Process -Id $app.Process.Id -Force -ErrorAction SilentlyContinue }

function Test-Responding($app) { $app.Process.Refresh(); $app.Process.Responding }

# Depth-first search by exact name. Native list views (thousands of rows) are
# skipped unless -IncludeLists, so a search stays fast.
function Find-Uia($app, [string]$Name, [switch]$IncludeLists, $Root = $null, [int]$Depth = 0) {
  if (-not $Root) { $Root = $app.Window }
  if ($Depth -gt 5) { return $null }
  $child = $Walker.GetFirstChild($Root)
  while ($child -ne $null) {
    if ($child.Current.Name -eq $Name) { return $child }
    if ($IncludeLists -or $child.Current.ClassName -ne 'SysListView32') {
      $hit = Find-Uia $app $Name -IncludeLists:$IncludeLists $child ($Depth + 1)
      if ($hit) { return $hit }
    }
    $child = $Walker.GetNextSibling($child)
  }
  return $null
}

function Show-UiaTree($app, [int]$MaxDepth = 2, [int]$MaxChildren = 40, $Root = $null, [int]$Depth = 0) {
  if (-not $Root) { $Root = $app.Window }
  if ($Depth -gt $MaxDepth) { return }
  $c = $Root.Current
  '{0}{1} ''{2}'' id=''{3}''' -f ('  ' * $Depth), $c.ControlType.ProgrammaticName.Replace('ControlType.', ''), $c.Name, $c.AutomationId
  $n = 0
  $child = $Walker.GetFirstChild($Root)
  while ($child -ne $null -and $n -lt $MaxChildren) {
    Show-UiaTree $app $MaxDepth $MaxChildren $child ($Depth + 1)
    $child = $Walker.GetNextSibling($child); $n++
  }
}

function Invoke-Uia($app, [string]$Name) {
  $el = Find-Uia $app $Name
  if (-not $el) { throw "no element named '$Name'" }
  $pattern = $null
  if ($el.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) { $pattern.Invoke(); return }
  if ($el.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$pattern)) { $pattern.Toggle(); return }
  if ($el.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$pattern)) { $pattern.Select(); return }
  throw "'$Name' supports no Invoke/Toggle/Select pattern"
}

function Click-Uia($app, [string]$Name) {
  $el = Find-Uia $app $Name
  if (-not $el) { throw "no element named '$Name'" }
  $r = $el.Current.BoundingRectangle
  [UiaInput]::SetCursorPos([int]($r.X + $r.Width / 2), [int]($r.Y + $r.Height / 2)) | Out-Null
  Start-Sleep -Milliseconds 300
  [UiaInput]::mouse_event(2, 0, 0, 0, 0); Start-Sleep -Milliseconds 80; [UiaInput]::mouse_event(4, 0, 0, 0, 0)
  Start-Sleep -Milliseconds 500
}

function Send-Key([byte]$VirtualKey) {
  [UiaInput]::keybd_event($VirtualKey, 0, 0, 0); Start-Sleep -Milliseconds 60; [UiaInput]::keybd_event($VirtualKey, 0, 2, 0)
  Start-Sleep -Milliseconds 500
}

# A minidump of a hung process (analyse it with `llvm-symbolizer` on the return
# addresses of the main thread's stack; see docs/win32-uia.md).
function Save-Dump($app, [string]$Path) {
  rundll32 C:\Windows\System32\comsvcs.dll, MiniDump $app.Process.Id $Path full
  Start-Sleep 10
  icacls $Path /grant "$($env:USERNAME):F" | Out-Null
}
