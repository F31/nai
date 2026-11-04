param([string]$BinaryDirectory = '')
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
if (!$BinaryDirectory) { $BinaryDirectory = Join-Path $project 'target-windows/live-auto-runtime' }
$binaryRoot = [IO.Path]::GetFullPath($BinaryDirectory)
$desktop = Join-Path $binaryRoot 'nai-workbench-desktop.exe'
$foundation = Join-Path $binaryRoot 'nai-foundation-engine.exe'
$inspection = Join-Path $binaryRoot 'nai-workbench-engine.exe'
$publicKeeper = Join-Path $binaryRoot 'nai-public-output-keeper.exe'
$audition = Join-Path $binaryRoot 'nai-audition-worker.exe'
$microphone = Join-Path $binaryRoot 'nai-human-mic-worker.exe'
foreach ($file in @($desktop, $foundation, $inspection, $audition, $publicKeeper, $microphone)) {
    if (!(Test-Path -LiteralPath $file -PathType Leaf)) { throw '请先构建新的备播工作台和引擎；此入口不会启动旧安装。' }
}
$info = [Diagnostics.ProcessStartInfo]::new()
$info.FileName = $desktop
$info.WorkingDirectory = $project
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.Environment['NAI_PUBLIC_OUTPUT_KEEPER'] = $publicKeeper
$info.Environment['NAI_FOUNDATION_ENGINE'] = $foundation
$info.Environment['NAI_WORKBENCH_ENGINE'] = $inspection
$info.Environment.Remove('NAI_HUMAN_MIC_WORKER') | Out-Null
# Test overrides must never redirect a normal manual session.
$info.Environment.Remove('NAI_WORKBENCH_TEST_ROOT') | Out-Null
$info.Environment.Remove('NAI_LLM_TEST_LOOPBACK') | Out-Null
$info.Environment.Remove('NAI_PUBLIC_OUTPUT_TEST_HIDDEN') | Out-Null
$process = [Diagnostics.Process]::Start($info)
if (!$process) { throw '备播工作台启动失败。' }
Write-Output ('已打开新备播工作台，进程 ' + $process.Id + '；旧安装和旧数据库保持独立。')
$process.Dispose()


