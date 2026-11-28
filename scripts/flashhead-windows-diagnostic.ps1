#requires -Version 7.0
<# Runs the official FlashHead help/import diagnostic with isolated dependencies. #>
param(
  [string]$PythonPath = '',
  [string]$UpstreamDir = '',
  [string]$DependencyDir = '',
  [string]$NumpyDir = '',
  [string]$TritonDir = ''
)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
if (!$PythonPath) { $PythonPath = Join-Path $project 'target-windows/realistic-avatar-research/venv/Scripts/python.exe' }
if (!$UpstreamDir) { $UpstreamDir = Join-Path $project 'target-windows/flashhead-upstream' }
if (!$DependencyDir) { $DependencyDir = Join-Path $project 'target-windows/flashhead-win-diagnostic/overlay' }
if (!$NumpyDir) { $NumpyDir = Join-Path $project 'target-windows/flashhead-win-diagnostic/numpy126' }
if (!$TritonDir) { $TritonDir = Join-Path $project 'target-windows/flashhead-win-diagnostic/triton331-isolated' }
foreach ($path in @($PythonPath, (Join-Path $UpstreamDir 'generate_video.py'), $DependencyDir, $NumpyDir)) {
  if (!(Test-Path -LiteralPath $path)) { throw "缺少诊断依赖：$path" }
}
. (Join-Path $project 'scripts/use-project-build-cache.ps1') -Target 'flashhead-win-diagnostic'
$env:PYTHONPATH = "$NumpyDir;$DependencyDir"
if (Test-Path -LiteralPath $TritonDir -PathType Container) { $env:PYTHONPATH += ";$TritonDir" }
$env:USE_TF = '0'
Push-Location $UpstreamDir
try {
  & $PythonPath generate_video.py --help
  if ($LASTEXITCODE -ne 0) { throw "FlashHead 导入诊断失败：退出码 $LASTEXITCODE" }
} finally { Pop-Location }
