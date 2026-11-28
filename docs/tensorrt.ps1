$dir = 'E:\projects\nai\target-windows\tool-cache\tensorrt-v561-download'
$name = 'tensorrt_cu12_libs-10.9.0.34-py2.py3-none-win_amd64.whl'
$part = Join-Path $dir "$name.part"
$url = "https://pypi.nvidia.com/tensorrt-cu12-libs/$name"
$expectedSize = 1627286840
$expectedHash = 'e43d38cc380d615bf8afab637c265f01a9452dc23deb9b9dd47b7662edf24531'

New-Item -ItemType Directory -Force -Path $dir | Out-Null
if (-not (Test-Path -LiteralPath $part)) {
    New-Item -ItemType File -Path $part | Out-Null
}

$stalled = 0
while ((Get-Item -LiteralPath $part).Length -lt $expectedSize) {
    $before = (Get-Item -LiteralPath $part).Length

    curl.exe --noproxy '*' --fail --location --continue-at - `
        --connect-timeout 20 --max-time 120 `
        --output $part $url

    $after = (Get-Item -LiteralPath $part).Length
    Write-Host ("Downloaded {0:N1} / {1:N1} MiB" -f ($after / 1MB), ($expectedSize / 1MB))

    if ($after -le $before) { $stalled++ } else { $stalled = 0 }
    if ($stalled -ge 3) {
        throw 'No progress after three attempts. Partial file retained; rerun this script to resume.'
    }
}

if ((Get-Item -LiteralPath $part).Length -ne $expectedSize) {
    throw 'File size mismatch. Do not install.'
}

Write-Host 'Download complete. Calculating SHA-256...'
$actualHash = (Get-FileHash -LiteralPath $part -Algorithm SHA256).Hash.ToLowerInvariant()

if ($actualHash -ne $expectedHash) {
    throw "SHA-256 mismatch: $actualHash. Do not install."
}

Move-Item -LiteralPath $part -Destination (Join-Path $dir $name)
Write-Host 'Download complete and SHA-256 verified.'
