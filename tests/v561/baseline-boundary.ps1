$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$parent = Join-Path $project 'target-windows/v561-baseline-tests'
New-Item -ItemType Directory -Force -Path $parent | Out-Null
$case = Join-Path $parent ([guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $case | Out-Null
try {
  $sentinel = Join-Path $case 'sentinel'
  Set-Content -LiteralPath $sentinel -Value 'preserve'
  $script = Join-Path $project 'scripts/v561-baseline.ps1'
  $output = & pwsh -NoProfile -File $script -DurationSeconds 10 -OutputDir $case 2>&1 | Out-String
  if ($LASTEXITCODE -eq 0 -or $output -notmatch 'NAI_BASELINE_OUTPUT_NOT_EMPTY') { throw 'Nonempty output directory was not rejected' }
  if ((Get-Content -Raw -LiteralPath $sentinel).Trim() -ne 'preserve' -or @(Get-ChildItem -LiteralPath $case -Force).Count -ne 1) { throw 'Prior evidence changed' }
  'PASS: nonempty output rejected; prior evidence unchanged'
} finally {
  if ((Split-Path -Parent $case) -ne $parent) { throw 'Unexpected test directory' }
  Remove-Item -LiteralPath $case -Recurse -Force
}
