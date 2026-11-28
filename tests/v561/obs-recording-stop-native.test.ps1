#requires -Version 7.0
param([Parameter(Mandatory)][string]$ObsBinary,[switch]$MonitorExit)
# Recorder control diagnostic only: no avatar, microphone, model API or live stream.
$ErrorActionPreference='Stop'
$root=Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$run=Join-Path $root ('target-windows/obs-stop-native/'+[guid]::NewGuid().ToString('N'))
$config=Join-Path (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $ObsBinary))) 'config/obs-studio'
$name='nai-stop-'+[guid]::NewGuid().ToString('N')
New-Item -ItemType Directory -Force "$run/native","$run/recordings","$config/basic/profiles/$name"|Out-Null
$template=Get-Content "$config/basic/scenes/nai-mixed-30min-20261118.json" -Raw|ConvertFrom-Json
$template.name=$name
$template.modules.'scripts-tool'=@()
$template.modules.'output-timer'.recordTimerHours=0
$template.modules.'output-timer'.recordTimerMinutes=0
$template.modules.'output-timer'.recordTimerSeconds=45
$template|ConvertTo-Json -Depth 100|Set-Content "$config/basic/scenes/$name.json" -Encoding utf8
$ini=Get-Content "$config/basic/profiles/nai-mixed-30min-20261118/basic.ini" -Raw
$ini=[regex]::Replace($ini,'(?m)^Name=.*$',"Name=$name")
$ini=[regex]::Replace($ini,'(?m)^(FilePath|RecFilePath|FFFilePath)=.*$',('$1='+$run.Replace('\','/')+'/recordings'))
$ini|Set-Content "$config/basic/profiles/$name/basic.ini" -Encoding utf8
'{"scope":"recording_control_only_no_av_readiness"}'|Set-Content "$run/native/mixed-ready.json"
$arguments=@{ObsBinary=$ObsBinary;Profile=$name;Collection=$name;RecordingDirectory="$run/recordings";ArtifactsDirectory="$run/native";MinimumRecordingSeconds=45}
if ($MonitorExit) {
    $fixture=Start-Process -FilePath (Join-Path $PSHOME 'pwsh.exe') -ArgumentList @('-NoProfile','-Command','Start-Sleep -Seconds 10; exit 7') -WindowStyle Hidden -PassThru
    $arguments.TestProcessId=$fixture.Id
}
& "$root/scripts/start-v561-isolated-recording.ps1" @arguments
$control=Get-Content "$run/native/obs-control.json" -Raw|ConvertFrom-Json
$ready=Get-Content "$run/native/obs.ready" -Raw|ConvertFrom-Json
if (!$MonitorExit) {
    [IO.File]::WriteAllText("$run/native/stop.request",('0'*32))
    Start-Sleep -Seconds 2
    if ((Test-Path "$run/native/stop.receipt") -or (Get-Content $ready.log -Raw) -match '==== Recording Stop') { throw 'Wrong nonce stopped recording.' }
} else {
    $fixture.WaitForExit()
    if ($fixture.ExitCode -ne 7 -or !(Test-Path "$run/native/obs.stopped")) { throw 'Failed test exit did not seal recording.' }
}
& "$root/scripts/stop-v561-isolated-recording.ps1" -ArtifactsDirectory "$run/native"
& "$root/scripts/stop-v561-isolated-recording.ps1" -ArtifactsDirectory "$run/native"
$text=Get-Content $ready.log -Raw
if ($text -match '\[Lua:.*(Error|Failed)' -or !$text.Contains('==== Recording Stop')) { throw 'Watchdog or sealing failed.' }
@{scope='recording_control_only_no_av_readiness';wrong_nonce_ignored=(!$MonitorExit);failed_test_auto_stop=[bool]$MonitorExit;normal_stop=$true;repeated_stop_idempotent=$true;file=$ready.file;pid=$control.pid;artifacts="$run/native"}|ConvertTo-Json|Set-Content "$run/result.json"
Write-Output "Recorder control passed. Evidence: $run/result.json"
