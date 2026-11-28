#requires -Version 7.0
param([Parameter(Mandatory)][string]$ArtifactsDirectory,[ValidateRange(1,60)][int]$TimeoutSeconds=15)
$ErrorActionPreference='Stop'
$allowed=[IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $PSScriptRoot) 'target-windows'))+'\'
$artifacts=(Resolve-Path -LiteralPath $ArtifactsDirectory).Path
if (!$artifacts.StartsWith($allowed,[StringComparison]::OrdinalIgnoreCase)) { throw 'Only project-local recording control is permitted.' }
$part=Get-Item -LiteralPath $artifacts -Force
while ($part -and $part.FullName.Length -ge $allowed.TrimEnd('\').Length) {
    if ($part.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse-point control paths are forbidden.' }
    $part=$part.Parent
}
$control=Get-Content -LiteralPath (Join-Path $artifacts 'obs-control.json') -Raw|ConvertFrom-Json
$ready=Get-Content -LiteralPath (Join-Path $artifacts 'obs.ready') -Raw|ConvertFrom-Json
if ($control.nonce -cnotmatch '^[a-f0-9]{32}$' -or $ready.pid -ne $control.pid -or $control.artifacts -ne $artifacts -or !$ready.recording_confirmed) { throw 'Invalid run identity.' }
$binary=[IO.Path]::GetFullPath($control.binary)
if (!$binary.StartsWith($allowed,[StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($binary) -ne 'obs64.exe') { throw 'Invalid isolated binary.' }
$process=Get-Process -Id ([int]$control.pid) -ErrorAction SilentlyContinue
if (!$process -or $process.Path -ne $binary -or [Math]::Abs(($process.StartTime.ToUniversalTime()-[datetime]$control.started_utc).TotalSeconds) -gt 5) { throw 'Isolated process identity changed.' }
foreach ($path in @($ready.log,$ready.file)) {
    if (![IO.Path]::GetFullPath($path).StartsWith($allowed,[StringComparison]::OrdinalIgnoreCase)) { throw 'Recording evidence outside project.' }
}
[IO.File]::WriteAllText((Join-Path $artifacts 'stop.request'),$control.nonce,[Text.UTF8Encoding]::new($false))
$watch=[Diagnostics.Stopwatch]::StartNew()
do {
    $receipt=Join-Path $artifacts 'stop.receipt'
    if ((Test-Path -LiteralPath $receipt) -and (Get-Item -LiteralPath $receipt).Length -eq 32 -and [IO.File]::ReadAllText($receipt) -ceq $control.nonce) {
        $log=Get-Content -LiteralPath $ready.log -Raw
        if ($log -match '==== Recording Stop' -and (Get-Item -LiteralPath $ready.file).Length -gt 0) {
            @{recording_stopped=$true;pid=$control.pid;file=$ready.file;utc=[DateTime]::UtcNow.ToString('o');stop_wait_ms=$watch.ElapsedMilliseconds} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $artifacts 'obs.stopped') -Encoding utf8
            Write-Output 'Isolated OBS recording stopped normally; file preserved.'
            return
        }
    }
    Start-Sleep -Milliseconds 100
} while ($watch.Elapsed.TotalSeconds -lt $TimeoutSeconds)
throw 'Normal recording stop not confirmed; preserve OBS and recording. No process was killed.'
