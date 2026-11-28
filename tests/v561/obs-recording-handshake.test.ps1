#requires -Version 7.0
# Validation fixtures only: no OBS, recording or model process is started.
$ErrorActionPreference='Stop'
$root=Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$dir=Join-Path $root ('target-windows/obs-handshake-tests/'+[guid]::NewGuid().ToString('N'))
$obs=Join-Path $dir 'obs'
New-Item -ItemType Directory -Force "$obs/bin/64bit","$obs/config/obs-studio/basic/profiles/test","$obs/config/obs-studio/basic/scenes","$dir/recordings","$dir/native"|Out-Null
Set-Content "$obs/bin/64bit/obs64.exe" 'validation fixture; never execute'
Set-Content "$dir/native/mixed-ready.json" '{}'
$scene=@{sources=@(@{id='window_capture';settings=@{window='nai:minifb_window:v561_live_control.exe'}},@{id='wasapi_process_output_capture';settings=@{window='nai:minifb_window:v561_live_control.exe'}});modules=@{'output-timer'=@{recordTimerHours=0;recordTimerMinutes=5;recordTimerSeconds=20;autoStartRecordTimer=$true;autoStartStreamTimer=$false};'scripts-tool'=@();'auto-scene-switcher'=@{active=$false}}}
$scenePath="$obs/config/obs-studio/basic/scenes/test.json"
function Save-Scene { $scene|ConvertTo-Json -Depth 10|Set-Content $scenePath -Encoding utf8 }
Save-Scene
"[Output]`nMode=Simple`n[SimpleOutput]`nFilePath=$($dir.Replace('\','/'))/recordings"|Set-Content "$obs/config/obs-studio/basic/profiles/test/basic.ini" -Encoding utf8
$arguments=@{ObsBinary="$obs/bin/64bit/obs64.exe";Profile='test';Collection='test';RecordingDirectory="$dir/recordings";ArtifactsDirectory="$dir/native";ValidateOnly=$true}
$script=Join-Path $root 'scripts/start-v561-isolated-recording.ps1'
& $script @arguments
function Reject([string]$name){$rejected=$false;try{& $script @arguments|Out-Null}catch{$rejected=$true};if(!$rejected){throw "Expected rejection: $name"};Write-Output "Rejected: $name"}
$scene.sources+=@{id='wasapi_input_capture'};Save-Scene;Reject 'microphone source'
$scene.sources=$scene.sources[0..1];$scene.modules.'output-timer'.autoStartRecordTimer=$false;Save-Scene;Reject 'unbounded recording'
$scene.modules.'output-timer'.autoStartRecordTimer=$true;Save-Scene
$scene.modules.'scripts-tool'=@(@{path='untrusted.lua'});Save-Scene;Reject 'untrusted script'
$scene.modules.'scripts-tool'=@();Save-Scene
Set-Content "$dir/native/stop.request" 'old-run';Reject 'stale stop request'
Remove-Item -LiteralPath "$dir/native/stop.request"
Set-Content "$dir/native/obs.ready" '{}';Reject 'stale ready signal'
$arguments.ArtifactsDirectory="$dir/recordings";Reject 'missing native preparation'
$arguments.ArtifactsDirectory="$dir/native";$arguments.ObsBinary='C:/Windows/System32/cmd.exe';Reject 'outside project path'
$arguments.ObsBinary="$obs/bin/64bit/obs64.exe"
Remove-Item -LiteralPath "$dir/native/obs.ready"
$arguments.CaptureAvDiagnostic=$true
Reject 'workbench sources in diagnostic mode'
foreach ($source in $scene.sources) {
    $source.settings.window='nai V5.6.1 - local AV diagnostic:minifb_window:nai-v561-av-diagnostic.exe'
    $source.settings.priority=1
}
Save-Scene
& $script @arguments
$scene.sources[0].settings.priority=0;Save-Scene;Reject 'window class fallback'
$scene.sources[0].settings.priority=1
$audioTarget=$scene.sources[1].settings.window
$scene.sources[1].settings.window='different:minifb_window:nai-v561-av-diagnostic.exe';Save-Scene;Reject 'different audio target'
$scene.sources[1].settings.window=$audioTarget
$scene.sources[0].settings.capture_audio=$true;Save-Scene;Reject 'duplicate window audio'
$scene.sources[0].settings.capture_audio=$false;Save-Scene
$arguments.ValidateOnly=$false;Reject 'missing live diagnostic PID'
$arguments.ValidateOnly=$true
$arguments.Remove('CaptureAvDiagnostic')
$arguments.LoopbackMediaPort=18673
$arguments.LoopbackWaitSeconds=45
$scene.sources=@(@{id='ffmpeg_source';settings=@{is_local_file=$false;input='udp://127.0.0.1:18673?timeout=45000000';input_format='mpegts'}})
Save-Scene
& $script @arguments
$scene.sources[0].settings.input='udp://127.0.0.1:18673?timeout=1000000';Save-Scene;Reject 'loopback timeout mismatch'
$scene.sources[0].settings.input='udp://0.0.0.0:18673?timeout=45000000';Save-Scene;Reject 'non-loopback input'
Write-Output 'Seventeen validation scenarios passed; no process started.'
