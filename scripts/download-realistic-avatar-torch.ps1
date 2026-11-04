param([int]$MaxSeconds = 300)
$ErrorActionPreference = 'Stop'
if ($MaxSeconds -lt 30 -or $MaxSeconds -gt 3600) { throw 'MaxSeconds must be between 30 and 3600.' }
$root = Join-Path (Split-Path $PSScriptRoot -Parent) 'target-windows\realistic-avatar-research'
$downloads = Join-Path $root 'downloads'
New-Item -ItemType Directory -Force $downloads | Out-Null
$file = Join-Path $downloads 'torch-2.7.1+cu128-cp310-cp310-win_amd64.whl'
$expected = '5174f02de8ca14df87c8e333c4c39cf3ce93a323c9d470d690301d110a053b3c'
$url = 'https://download-r2.pytorch.org/whl/cu128/torch-2.7.1%2Bcu128-cp310-cp310-win_amd64.whl'
if (Test-Path -LiteralPath $file) {
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $file).Hash.ToLowerInvariant()
    if ($hash -eq $expected) { Write-Output "Verified: $file"; exit 0 }
}
# Only this process bypasses all proxy routes. A partial file stays on E: for
# a later retry; it is never installed until the official digest matches.
& curl.exe --noproxy '*' --fail --location --continue-at - --retry 3 --retry-delay 3 --retry-max-time $MaxSeconds --connect-timeout 15 --speed-limit 1024 --speed-time 60 --max-time $MaxSeconds --output $file $url
if ($LASTEXITCODE -ne 0) { throw "Download incomplete (curl $LASTEXITCODE); partial file preserved for resume." }
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $file).Hash.ToLowerInvariant()
if ($hash -ne $expected) { throw 'Downloaded wheel digest mismatch; file preserved and must not be installed.' }
Write-Output "Verified: $file"
