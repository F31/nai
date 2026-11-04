# On-demand architecture graph; separate from CodeGraph's live code index.
param(
    [ValidateSet('refresh','refresh-docs','status','query','docs','path','explain','affected','report')]
    [string]$Action = 'status',
    [Parameter(ValueFromRemainingArguments=$true)][string[]]$Arguments
)
$ErrorActionPreference = 'Stop'
$naiGraphRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'use-project-build-cache.ps1') -Target live-auto-build
$naiGraphTools = Join-Path $naiGraphRoot 'target-windows/tool-cache/graphify'
$naiGraphPython = Join-Path $naiGraphTools 'venv-0.9.96/Scripts/python.exe'
$naiGraphOutput = Join-Path $naiGraphTools 'index-nai'
$naiGraphJson = Join-Path $naiGraphOutput 'graph.json'
if (!(Test-Path -LiteralPath $naiGraphPython)) { throw 'Graphify 0.9.96 is missing; see docs/code-intelligence.md.' }
$env:GRAPHIFY_OUT = $naiGraphOutput
$env:GRAPHIFY_MAX_WORKERS = '2'
$env:GRAPHIFY_QUERY_LOG_DISABLE = '1'
$env:PYTHONUTF8 = '1'
$env:PYTHONUNBUFFERED = '1'
$env:PYTHONDONTWRITEBYTECODE = '1'
$env:DO_NOT_TRACK = '1'
function Invoke-NaiGraphifyProcess([string[]]$Cli, [string]$LogTag) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $naiGraphPython
    $info.WorkingDirectory = $naiGraphRoot
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($arg in $Cli) { $info.ArgumentList.Add($arg) }
    $process = [Diagnostics.Process]::Start($info)
    try {
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (!$process.WaitForExit(300000)) {
            $process.Kill($true)
            throw "Graphify $LogTag exceeded 300 seconds; inspect the graph before retrying."
        }
        $stdout.Result | Set-Content -LiteralPath (Join-Path $naiGraphTools "$LogTag-stdout.log") -Encoding utf8
        $stderr.Result | Set-Content -LiteralPath (Join-Path $naiGraphTools "$LogTag-stderr.log") -Encoding utf8
        Get-Content -LiteralPath (Join-Path $naiGraphTools "$LogTag-stdout.log") -Tail 12
        if ($stderr.Result) { Get-Content -LiteralPath (Join-Path $naiGraphTools "$LogTag-stderr.log") -Tail 8 }
        if ($process.ExitCode -ne 0) { throw "Graphify $LogTag failed; see the E: tool-cache logs." }
    } finally { $process.Dispose() }
}
Push-Location $naiGraphRoot
try {
    if ($Action -eq 'refresh') {
        if ($Arguments.Count) { throw 'refresh has fixed local-only options; extra arguments are not accepted.' }
        New-Item -ItemType Directory -Path $naiGraphOutput -Force | Out-Null
        Invoke-NaiGraphifyProcess @('-m','graphify','extract',$naiGraphRoot,'--code-only','--cargo','--max-workers','2','--no-dedup') 'extract'
        Invoke-NaiGraphifyProcess @('-m','graphify','cluster-only',$naiGraphRoot,'--graph',$naiGraphJson,'--no-label','--no-viz') 'cluster'
        Invoke-NaiGraphifyProcess @((Join-Path $PSScriptRoot 'graphify-doc-structure.py'),'--root',$naiGraphRoot,'--output',$naiGraphOutput) 'docs'
        $naiGraphExit = 0
    } elseif ($Action -eq 'refresh-docs') {
        Invoke-NaiGraphifyProcess @((Join-Path $PSScriptRoot 'graphify-doc-structure.py'),'--root',$naiGraphRoot,'--output',$naiGraphOutput) 'docs'
        $naiGraphExit = 0
    } elseif ($Action -eq 'status') {
        if (!(Test-Path -LiteralPath $naiGraphJson)) { throw 'No current graph; run scripts/graphify.ps1 refresh.' }
        $graph = Get-Content -LiteralPath $naiGraphJson -Raw | ConvertFrom-Json -Depth 100
        [pscustomobject]@{ Version=$graph.graph.graphify_version; BuiltAt=(Get-Item $naiGraphJson).LastWriteTime; Graph=$naiGraphJson; Nodes=$graph.nodes.Count; Edges=if($null -ne $graph.links){$graph.links.Count}else{$graph.edges.Count}; SemanticAnalysis='not performed' }
        $naiGraphExit = 0
    } elseif ($Action -eq 'report') {
        Get-Content -LiteralPath (Join-Path $naiGraphOutput 'GRAPH_REPORT.md')
        $naiGraphExit = 0
    } else {
        if (!(Test-Path -LiteralPath $naiGraphJson)) { throw 'No current graph; run scripts/graphify.ps1 refresh.' }
        if ($Action -eq 'docs') {
            $summary = Get-Content -LiteralPath (Join-Path $naiGraphOutput 'document-structure-summary.json') -Raw | ConvertFrom-Json
            foreach ($source in $summary.sources) {
                $current = (Get-FileHash -LiteralPath (Join-Path $naiGraphRoot $source.path) -Algorithm SHA256).Hash
                if ($current -ne $source.sha256) { throw '文档已变化，请先运行 scripts/graphify.ps1 refresh-docs。' }
            }
            if ((Get-FileHash -LiteralPath $naiGraphJson -Algorithm SHA256).Hash -ne $summary.code_graph_sha256) { throw '代码图已刷新，请先运行 scripts/graphify.ps1 refresh-docs。' }
            $Action = 'query'
            $naiGraphJson = Join-Path $naiGraphOutput 'architecture-docs.json'
        }
        & $naiGraphPython -m graphify $Action @Arguments --graph $naiGraphJson
        $naiGraphExit = $LASTEXITCODE
    }
} finally { Pop-Location }
exit $naiGraphExit
