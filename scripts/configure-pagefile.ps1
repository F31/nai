#requires -RunAsAdministrator
[CmdletBinding()]
param(
  [ValidateRange(1024, 1048576)][uint32]$InitialMiB = 16384,
  [ValidateRange(1024, 1048576)][uint32]$MaximumMiB = 32768,
  [string]$TargetDrive = '',
  [string]$RecordDir = ''
)
$ErrorActionPreference = 'Stop'
if ($MaximumMiB -lt $InitialMiB) { throw 'MaximumMiB must be at least InitialMiB.' }
$project = Split-Path -Parent $PSScriptRoot
if (!$TargetDrive) { $TargetDrive = [IO.Path]::GetPathRoot($project).TrimEnd('\') }
if ($TargetDrive -notmatch '^[A-Za-z]:$') { throw 'TargetDrive must be a local drive letter.' }
$systemDrive = $env:SystemDrive.ToUpperInvariant()
$TargetDrive = $TargetDrive.ToUpperInvariant()
if ($TargetDrive -eq $systemDrive) { throw 'TargetDrive must differ from the system drive.' }
$disk = Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='$TargetDrive'"
if (!$disk -or $disk.DriveType -ne 3 -or $disk.FileSystem -ne 'NTFS') { throw 'Target must be a fixed NTFS disk.' }
if ($disk.FreeSpace -lt (($InitialMiB + 4096) * 1MB)) { throw 'Target drive needs initial pagefile size plus 4 GiB free.' }
$system = Get-CimInstance Win32_ComputerSystem
$settings = @(Get-CimInstance Win32_PageFileSetting)
if (!$system.AutomaticManagedPagefile -or $settings.Count -gt 0) { throw 'Pagefile configuration changed since inspection; refusing to overwrite it.' }
if (!$RecordDir) { $RecordDir = Join-Path $project 'target-windows/pagefile-maintenance' }
New-Item -ItemType Directory -Force -Path $RecordDir | Out-Null
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$record = Join-Path $RecordDir "pagefile-$stamp.json"
$before = [ordered]@{ timestamp=(Get-Date).ToString('o'); automatic=$true; settings=$settings; usage=@(Get-CimInstance Win32_PageFileUsage | Select-Object Name,AllocatedBaseSize,CurrentUsage); target_drive=$TargetDrive; initial_mib=$InitialMiB; maximum_mib=$MaximumMiB; status='started' }
$before | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $record -Encoding UTF8
$created = @()
try {
  Set-CimInstance -InputObject $system -Property @{ AutomaticManagedPagefile = $false } | Out-Null
  $current = @(Get-CimInstance Win32_PageFileSetting)
  if ($current | Where-Object { $_.Name -ieq "$TargetDrive\pagefile.sys" }) { throw 'Target pagefile already exists; refusing to overwrite it.' }
  if (!($current | Where-Object { $_.Name -ieq "$systemDrive\pagefile.sys" })) {
    $c = New-CimInstance -ClassName Win32_PageFileSetting -Property @{ Name="$systemDrive\pagefile.sys"; InitialSize=[uint32]0; MaximumSize=[uint32]0 }
    $created += $c
  }
  $e = New-CimInstance -ClassName Win32_PageFileSetting -Property @{ Name="$TargetDrive\pagefile.sys"; InitialSize=$InitialMiB; MaximumSize=$MaximumMiB }
  $created += $e
  $after = @(Get-CimInstance Win32_PageFileSetting)
  if (!($after | Where-Object { $_.Name -ieq "$systemDrive\pagefile.sys" }) -or !($after | Where-Object { $_.Name -ieq "$TargetDrive\pagefile.sys" -and $_.InitialSize -eq $InitialMiB -and $_.MaximumSize -eq $MaximumMiB })) { throw 'Pagefile configuration verification failed.' }
  $before['status']='configured_restart_required'; $before['after']=$after | Select-Object Name,InitialSize,MaximumSize
} catch {
  $before['status']='failed'; $before['error']=$_.Exception.Message
  foreach ($item in $created) { try { Remove-CimInstance -InputObject $item -ErrorAction Stop } catch { $before['rollback_error'] = $_.Exception.Message } }
  try { Set-CimInstance -InputObject (Get-CimInstance Win32_ComputerSystem) -Property @{ AutomaticManagedPagefile = $true } | Out-Null } catch { $before['rollback_error'] = $_.Exception.Message }
  throw
} finally {
  $before | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $record -Encoding UTF8
}
Write-Output "Configured; restart required. Record: $record"

