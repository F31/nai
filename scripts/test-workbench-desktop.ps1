param(
    [Parameter(Mandatory=$true)][string]$Desktop,
    [Parameter(Mandatory=$true)][string]$Engine,
    [Parameter(Mandatory=$true)][string]$Foundation
)
$ErrorActionPreference = 'Stop'
foreach ($file in @($Desktop, $Engine, $Foundation)) {
    if (!(Test-Path -LiteralPath $file -PathType Leaf) -or ![IO.Path]::IsPathFullyQualified($file)) { throw 'RF01_BINARY_REQUIRED' }
}
# Verify the shipped image's PE subsystem, rather than relying only on source attributes.
$image = [IO.File]::ReadAllBytes($Desktop)
$pe = [BitConverter]::ToInt32($image, 0x3c)
if ([BitConverter]::ToUInt16($image, $pe + 24 + 68) -ne 2) { throw 'RF01_DESKTOP_REQUIRES_GUI_SUBSYSTEM' }
$info = [Diagnostics.ProcessStartInfo]::new()
$info.FileName = $Desktop
$info.ArgumentList.Add('--smoke')
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
$info.Environment['NAI_WORKBENCH_ENGINE'] = $Engine
$info.Environment['NAI_FOUNDATION_ENGINE'] = $Foundation
$info.Environment['NAI_WORKBENCH_TEST_ROOT'] = Join-Path ([IO.Path]::GetTempPath()) ('nai-workbench-smoke-' + [guid]::NewGuid().ToString('N'))
$process = [Diagnostics.Process]::new()
$process.StartInfo = $info
try {
    if (!$process.Start()) { throw 'RF01_DESKTOP_START_FAILED' }
    $output = $process.StandardOutput.ReadToEndAsync()
    $errors = $process.StandardError.ReadToEndAsync()
    if (!$process.WaitForExit(30000)) { $process.Kill($true); $process.WaitForExit(); throw 'RF01_DESKTOP_SMOKE_TIMEOUT' }
    $text = $output.GetAwaiter().GetResult()
    $safeErrors = $errors.GetAwaiter().GetResult()
    if ($process.ExitCode -ne 0 -or !$text.Contains('WORKBENCH_PREPARATION_READY: authenticated owner protocol=2 live_ready=false')) {
        throw ('RF01_DESKTOP_SMOKE_FAILED: exit=' + $process.ExitCode + '; ' + $safeErrors)
    }
    Write-Output $text.Trim()
} finally {
    if ($process.Id -and !$process.HasExited) { $process.Kill($true); $process.WaitForExit() }
    $process.Dispose()
}
