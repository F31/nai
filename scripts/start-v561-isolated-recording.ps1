#requires -Version 7.0
# Recording and ready acknowledgement are one bounded operation.
param(
    [Parameter(Mandatory)][string]$ObsBinary,
    [Parameter(Mandatory)][ValidatePattern('^[a-zA-Z0-9_-]+$')][string]$Profile,
    [Parameter(Mandatory)][ValidatePattern('^[a-zA-Z0-9_-]+$')][string]$Collection,
    [Parameter(Mandatory)][string]$RecordingDirectory,
    [Parameter(Mandatory)][string]$ArtifactsDirectory,
    [ValidateRange(1,3600)][int]$MinimumRecordingSeconds = 315,
    [ValidateRange(0,2147483647)][int]$TestProcessId = 0,
    [string]$LocalMediaFixture,
    [ValidateRange(1024,65535)][int]$LoopbackMediaPort,
    [ValidateRange(1,240)][int]$LoopbackWaitSeconds = 1,
    [switch]$RequireMediaLease,
    [switch]$CaptureAvDiagnostic,
    [switch]$ValidateOnly
)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
$allowed = [IO.Path]::GetFullPath((Join-Path $project 'target-windows')) + [IO.Path]::DirectorySeparatorChar
function Resolve-Local([string]$value) {
    if (![IO.Path]::IsPathRooted($value)) { throw 'Absolute project-local path required.' }
    $path = (Resolve-Path -LiteralPath $value).Path
    if (!$path.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) { throw 'Only project target-windows paths are permitted.' }
    $part=Get-Item -LiteralPath $path -Force
    while ($part) {
        if ($part.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse-point paths are not permitted for recording.' }
        if ([string]::Equals($part.FullName.TrimEnd('\'),$allowed.TrimEnd('\'),[StringComparison]::OrdinalIgnoreCase)) { break }
        $part=if($part -is [IO.DirectoryInfo]){$part.Parent}else{$part.Directory}
    }
    return $path
}
$binary = Resolve-Local $ObsBinary
if ([IO.Path]::GetFileName($binary) -ne 'obs64.exe') { throw 'Expected isolated obs64.exe.' }
$recordings = Resolve-Local $RecordingDirectory
$artifacts = Resolve-Local $ArtifactsDirectory
if (!(Test-Path -LiteralPath (Join-Path $artifacts 'mixed-ready.json'))) { throw 'Native preparation acknowledgement is missing.' }
$readyPath = Join-Path $artifacts 'obs.ready'
if ((Test-Path -LiteralPath $readyPath) -or (Test-Path -LiteralPath "$readyPath.tmp")) { throw 'Ready acknowledgement already exists; refusing replay.' }
$obsRoot = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $binary))
$config = Join-Path $obsRoot 'config/obs-studio'
$scenePath = Resolve-Local (Join-Path $config "basic/scenes/$Collection.json")
$profilePath = Resolve-Local (Join-Path $config "basic/profiles/$Profile/basic.ini")
$scene = Get-Content -LiteralPath $scenePath -Raw -Encoding UTF8 | ConvertFrom-Json
if (@($scene.modules.'scripts-tool').Count -gt 0 -or $scene.modules.'auto-scene-switcher'.active) { throw 'Scripts and automatic scene switching must be disabled.' }
$timer = $scene.modules.'output-timer'
$seconds = $timer.recordTimerHours * 3600 + $timer.recordTimerMinutes * 60 + $timer.recordTimerSeconds
if (!$timer.autoStartRecordTimer -or $timer.autoStartStreamTimer -or $seconds -lt $MinimumRecordingSeconds) { throw 'A sufficient automatic recording stop timer is required; stream timer must be disabled.' }
if ($LocalMediaFixture -and $LoopbackMediaPort) { throw 'Select one media fixture mode.' }
if ($RequireMediaLease -and !$LoopbackMediaPort) { throw 'Media lease requires a loopback fixture.' }
if ($CaptureAvDiagnostic -and ($LocalMediaFixture -or $LoopbackMediaPort)) { throw 'Diagnostic capture requires window and process audio sources.' }
if ($CaptureAvDiagnostic -and !$ValidateOnly -and !$TestProcessId) { throw 'A live diagnostic PID is required.' }
if ($LocalMediaFixture -or $LoopbackMediaPort) {
    $sources = @($scene.sources | Where-Object id -eq 'ffmpeg_source')
    if ($sources.Count -ne 1 -or @($scene.sources | Where-Object {$_.id -notin @('ffmpeg_source','scene')}).Count -ne 0) {
        throw 'Expected one isolated local media source.'
    }
    if ($LocalMediaFixture) {
        $media = Resolve-Local $LocalMediaFixture
        if (!$sources[0].settings.is_local_file -or (Resolve-Local $sources[0].settings.local_file) -ne $media) {
            throw 'Unexpected local fixture.'
        }
    } elseif ($sources[0].settings.is_local_file -or
        $sources[0].settings.input -cne "udp://127.0.0.1:$($LoopbackMediaPort)?timeout=$($LoopbackWaitSeconds * 1000000)" -or
        $sources[0].settings.input_format -cne 'mpegts') {
        throw 'Unexpected loopback media fixture.'
    }
} else {
$allowedSources = @('window_capture', 'wasapi_process_output_capture', 'scene')
$diagnosticTitle = 'nai V5.6.1 - local AV diagnostic'
$diagnosticExe = 'nai-v561-av-diagnostic.exe'
foreach ($source in $scene.sources) {
    if ($source.id -notin $allowedSources) { throw 'Only the public window and its process audio are permitted.' }
    if ($source.id -eq 'scene') { continue }
    if ($CaptureAvDiagnostic) {
        if ($source.settings.window -cne "${diagnosticTitle}:minifb_window:$diagnosticExe" -or
            $source.settings.priority -ne 1 -or
            $source.settings.capture_audio) { throw 'Expected exact diagnostic title matching and separate process audio.' }
    } elseif (!$source.settings.window.EndsWith(':v561_live_control.exe')) { throw 'Unexpected capture target.' }
}
foreach ($required in @('window_capture','wasapi_process_output_capture')) {
    if (@($scene.sources | Where-Object id -eq $required).Count -ne 1) { throw 'Exactly one window and one process audio source are required.' }
}
}
$ini = Get-Content -LiteralPath $profilePath -Raw -Encoding UTF8
if ($ini -notmatch '(?s)\[Output\][^\[]*\bMode=Simple\b') { throw 'This recorder supports the validated Simple output profile only.' }
$forward = $recordings.Replace('\','/')
if ($ini -notmatch ('(?m)^FilePath=' + [regex]::Escape($forward) + '\r?$')) { throw 'Profile recording directory differs from this run.' }
foreach ($name in @('obs-control.json','stop.request','stop.receipt','obs.stopped','recording-watchdog.lua','media.lease')) {
    if (Test-Path -LiteralPath (Join-Path $artifacts $name)) { throw 'Recording control artifacts already exist; use a new run directory.' }
}
if ($ValidateOnly) { Write-Output 'Isolated recording configuration validated; no process started.'; return }
$testProcess = if ($TestProcessId) { Get-Process -Id $TestProcessId -ErrorAction Stop } else { $null }
if ($CaptureAvDiagnostic) {
    $diagnosticPath = Resolve-Local $testProcess.Path
    if ([IO.Path]::GetFileName($diagnosticPath) -cne $diagnosticExe -or
        $testProcess.MainWindowTitle -cne $diagnosticTitle) { throw 'PID does not own the expected project diagnostic window.' }
    $matching = @(Get-Process -Name $testProcess.ProcessName | Where-Object MainWindowTitle -ceq $diagnosticTitle)
    if ($matching.Count -ne 1 -or $matching[0].Id -ne $TestProcessId) { throw 'Diagnostic capture target is ambiguous.' }
}
if (Get-Process -Name obs64 -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $binary }) { throw 'This isolated OBS is already running; no command was sent.' }
$nonce = [guid]::NewGuid().ToString('N')
$watchdog = Join-Path $artifacts 'recording-watchdog.lua'
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'obs-v561-recording-watchdog.lua') -Destination $watchdog
$settings = @{artifacts=$artifacts.Replace('\','/');nonce=$nonce}
if ($RequireMediaLease) {
    $settings.media_source = $sources[0].name
    $sources[0].muted = $true
    foreach ($sceneSource in $scene.sources | Where-Object id -eq 'scene') {
        foreach ($item in $sceneSource.settings.items | Where-Object name -ceq $sources[0].name) { $item.visible = $false }
    }
}
$scene.modules.'scripts-tool' = @(@{path=$watchdog.Replace('\','/');settings=$settings})
Copy-Item -LiteralPath $scenePath -Destination (Join-Path $artifacts 'scene-before-watchdog.json')
$scene | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $scenePath -Encoding utf8
$started = Get-Date
$process = Start-Process -FilePath $binary -WorkingDirectory (Split-Path -Parent $binary) -ArgumentList @('--portable','--multi','--profile',$Profile,'--collection',$Collection,'--startrecording','--disable-shutdown-check','--disable-missing-files-check') -WindowStyle Hidden -PassThru
@{pid=$process.Id;profile=$Profile;collection=$Collection;started_utc=$started.ToUniversalTime().ToString('o')} | ConvertTo-Json | Set-Content (Join-Path $artifacts 'obs-process.json') -Encoding utf8
@{pid=$process.Id;binary=$binary;profile=$Profile;collection=$Collection;nonce=$nonce;started_utc=$started.ToUniversalTime().ToString('o');artifacts=$artifacts} | ConvertTo-Json | Set-Content (Join-Path $artifacts 'obs-control.json') -Encoding utf8
$watch = [Diagnostics.Stopwatch]::StartNew()
$logDirectory = Join-Path $config 'logs'
while ($watch.Elapsed.TotalSeconds -lt 30) {
    if ($process.HasExited) { throw 'Isolated OBS exited before recording acknowledgement.' }
    $log = if (Test-Path -LiteralPath $logDirectory -PathType Container) {
        Get-ChildItem -LiteralPath $logDirectory -File | Where-Object { $_.LastWriteTime -ge $started.AddSeconds(-1) } | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    }
    if ($log) {
        $text = Get-Content -LiteralPath $log.FullName -Raw -Encoding UTF8
        $start = [regex]::Match($text, '(?m)^(\d{2}:\d{2}:\d{2}\.\d{3}): ==== Recording Start')
        $file = [regex]::Match($text, "Writing file '([^']+)'[.][.][.]")
        if ($start.Success -and $file.Success) {
            $actual = [IO.Path]::GetFullPath($file.Groups[1].Value)
            if (![string]::Equals([IO.Path]::GetDirectoryName($actual),$recordings,[StringComparison]::OrdinalIgnoreCase)) { throw 'OBS wrote outside this recording directory; no handshake sent.' }
            $recordAt = $started.Date.Add([TimeSpan]::Parse($start.Groups[1].Value))
            if ($recordAt -lt $started.AddMinutes(-1)) { $recordAt=$recordAt.AddDays(1) }
            $delay = ((Get-Date)-$recordAt).TotalMilliseconds
            if ($delay -lt 0 -or $delay -gt 3000) { throw 'Recording acknowledgement is stale; no handshake sent.' }
            $ready = @{recording_confirmed=$true;utc=[DateTime]::UtcNow.ToString('o');recording_started=$recordAt.ToString('o');handshake_delay_ms=$delay;file=$actual;pid=$process.Id;log=$log.FullName;timer_seconds=$seconds}
            if ($RequireMediaLease) { $ready.media_nonce = $nonce; $ready.media_port = $LoopbackMediaPort }
            $ready | ConvertTo-Json | Set-Content -LiteralPath "$readyPath.tmp" -Encoding utf8
            [IO.File]::Move("$readyPath.tmp", $readyPath)
            Write-Output ('Recording confirmed; ready acknowledged in {0:N0} ms.' -f $delay)
            if ($testProcess) {
                # Remain the recorder owner until completion/failure. The timer is a fallback.
                try {
                    $monitor=[Diagnostics.Stopwatch]::StartNew()
                    while (!$testProcess.HasExited -and !(Test-Path -LiteralPath (Join-Path $artifacts 'mixed-complete.json')) -and $monitor.Elapsed.TotalSeconds -lt $seconds) {
                        Start-Sleep -Milliseconds 200
                    }
                } finally {
                    & (Join-Path $PSScriptRoot 'stop-v561-isolated-recording.ps1') -ArtifactsDirectory $artifacts
                }
            }
            return
        }
    }
    Start-Sleep -Milliseconds 100
}
throw 'Recording acknowledgement timeout. No ready signal was written; preserve the isolated recording for diagnosis.'
