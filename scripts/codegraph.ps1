# Project-local CodeGraph. No product runtime dependency or global PATH mutation.
param([Parameter(ValueFromRemainingArguments=$true)][string[]]$Arguments)
$ErrorActionPreference = 'Stop'
$naiGraphRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'use-project-build-cache.ps1') -Target live-auto-build
$naiGraphTools = Join-Path $naiGraphRoot 'target-windows/tool-cache/codegraph'
$naiGraphBundle = Join-Path $naiGraphTools 'v1.6.2/codegraph-win32-x64'
$naiGraphNode = Join-Path $naiGraphBundle 'node.exe'
$naiGraphCli = Join-Path $naiGraphBundle 'lib/dist/bin/codegraph.js'
if (!(Test-Path -LiteralPath $naiGraphNode) -or !(Test-Path -LiteralPath $naiGraphCli)) {
    throw 'CodeGraph v1.6.2 is missing; see docs/codegraph.md for the verified installation.'
}
$env:CODEGRAPH_TELEMETRY = '0'
$env:DO_NOT_TRACK = '1'
$env:CODEGRAPH_NO_UPDATE_CHECK = '1'
$env:CODEGRAPH_NO_DAEMON = '1'
$env:CODEGRAPH_DIR = '.codegraph'
$env:NODE_COMPILE_CACHE = Join-Path $naiGraphTools 'node-cache'
if (!$Arguments.Count) { $Arguments = @('status') }
Push-Location $naiGraphRoot
try {
    & $naiGraphNode --liftoff-only --disable-warning=ExperimentalWarning $naiGraphCli @Arguments
    $naiGraphExit = $LASTEXITCODE
} finally { Pop-Location }
exit $naiGraphExit
