#requires -Version 7.0
<# Explicit local output-safety UI; not the unfinished full live workbench. #>
param([string]$Config='', [string]$RuntimeDir='', [string]$DesktopBinary='')
$ErrorActionPreference='Stop'
$naiRoot=Split-Path -Parent $PSScriptRoot
if(!$RuntimeDir){$RuntimeDir=Join-Path $naiRoot 'target-windows/v561-owner-late/debug'}
if(!$DesktopBinary){$DesktopBinary=Join-Path $naiRoot 'target-windows/build-desktop/debug/nai-v561-voice-workbench.exe'}
$naiRuntime=(Resolve-Path -LiteralPath $RuntimeDir).Path
$naiDesktop=(Resolve-Path -LiteralPath $DesktopBinary).Path
foreach($naiName in @('nai-v561-audition-control.exe','nai-v561-stream-audio-output.exe')){if(!(Test-Path -LiteralPath (Join-Path $naiRuntime $naiName) -PathType Leaf)){throw "NAI_V561_VOICE_PROGRAM_MISSING: $naiName"}}
$naiArguments='--runtime-dir "'+$naiRuntime+'"'
if($Config){$naiConfigPath=(Resolve-Path -LiteralPath $Config).Path;$naiArguments+=' --config "'+$naiConfigPath+'"'}
Start-Process -FilePath $naiDesktop -ArgumentList $naiArguments
