#requires -Version 7.0
<#
.SYNOPSIS
  Uninstall the V5.6.1 program while optionally keeping the single data root.
#>
param(
  [string]$DataRoot = "",
  [string]$RuntimeDir = "",
  [switch]$RemoveData
)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($DataRoot)) { $DataRoot = Join-Path $env:LOCALAPPDATA 'nai-v561' }
if ([string]::IsNullOrWhiteSpace($RuntimeDir)) { $RuntimeDir = Join-Path $project 'target-windows/v561-runtime' }
$DataRoot = [IO.Path]::GetFullPath($DataRoot)
$RuntimeDir = [IO.Path]::GetFullPath($RuntimeDir)
foreach ($path in @($DataRoot, $RuntimeDir)) {
  if ($path -eq [IO.Path]::GetPathRoot($path)) { throw "拒绝删除根目录: $path" }
}
$names = @('nai-workbench-desktop', 'nai-foundation-engine', 'nai-workbench-engine', 'nai-public-output-keeper', 'nai-v561-probe')
foreach ($name in $names) {
  $processes = Get-Process -Name $name -ErrorAction SilentlyContinue
  if ($processes) {
    $processes | Stop-Process -Force
    Write-Output "已停止 $name"
  }
}
Start-Sleep -Milliseconds 300
if (Test-Path -LiteralPath $RuntimeDir) {
  Remove-Item -LiteralPath $RuntimeDir -Recurse -Force
  Write-Output "已删除程序目录 $RuntimeDir"
} else {
  Write-Output "程序目录不存在，跳过: $RuntimeDir"
}
if ($RemoveData) {
  if (Test-Path -LiteralPath $DataRoot) {
    Remove-Item -LiteralPath $DataRoot -Recurse -Force
    Write-Output "已删除数据根 $DataRoot"
  }
} else {
  Write-Output "保留数据根 $DataRoot；重新安装后可直接复用。"
}
