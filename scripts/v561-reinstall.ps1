#requires -Version 7.0
<#
.SYNOPSIS
  Copy a built V5.6.1 runtime into the runtime directory and launch it.
  The data root is kept by default; -RemoveData starts clean.
#>
param(
  [string]$RuntimeSource = "",
  [string]$RuntimeDir = "",
  [string]$DataRoot = "",
  [switch]$RemoveData,
  [switch]$Build
)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($RuntimeSource)) { $RuntimeSource = Join-Path $project 'target-windows/build/release' }
if ([string]::IsNullOrWhiteSpace($RuntimeDir)) { $RuntimeDir = Join-Path $project 'target-windows/v561-runtime' }
if ([string]::IsNullOrWhiteSpace($DataRoot)) { $DataRoot = Join-Path $env:LOCALAPPDATA 'nai-v561' }
$RuntimeSource = [IO.Path]::GetFullPath($RuntimeSource)
$RuntimeDir = [IO.Path]::GetFullPath($RuntimeDir)
$DataRoot = [IO.Path]::GetFullPath($DataRoot)

if ($Build) {
  Push-Location $project
  try {
    cargo build --release -p nai-workbench-engine -p nai-public-output-keeper --bins
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
  } finally { Pop-Location }
}

& (Join-Path $PSScriptRoot 'v561-uninstall.ps1') -DataRoot $DataRoot -RuntimeDir $RuntimeDir -RemoveData:$RemoveData
New-Item -ItemType Directory -Force -Path $RuntimeDir | Out-Null

$files = @(
  'nai-workbench-desktop.exe',
  'nai-foundation-engine.exe',
  'nai-workbench-engine.exe',
  'nai-public-output-keeper.exe',
  'nai-v561-probe.exe'
)
foreach ($file in $files) {
  $source = Join-Path $RuntimeSource $file
  if (!(Test-Path -LiteralPath $source -PathType Leaf)) { throw "缺少构建产物 $source；先构建对应 binary。" }
  Copy-Item -LiteralPath $source -Destination (Join-Path $RuntimeDir $file) -Force
}
Write-Output "已安装到 $RuntimeDir"
& (Join-Path $PSScriptRoot 'v561-start.ps1') -DataRoot $DataRoot -RuntimeDir $RuntimeDir
