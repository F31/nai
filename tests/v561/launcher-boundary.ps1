$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$launcher = Join-Path $project 'scripts/v561-start.ps1'
$parent = Join-Path $project 'target-windows/v561-launcher-tests'
New-Item -ItemType Directory -Force -Path $parent | Out-Null
$case = Join-Path $parent ([guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $case | Out-Null
try {
  $root = Join-Path $case 'new-data'
  foreach ($flags in @(@(), @('-AllowDegraded'), @('-NoProbe'), @('-Build'), @('-AllowDegraded','-NoProbe','-Build'))) {
    $output = & pwsh -NoProfile -File $launcher -DataRoot $root -RuntimeDir $case @flags 2>&1 | Out-String
    if ($LASTEXITCODE -eq 0 -or $output -notmatch 'NAI_V561_ASSEMBLY_NOT_READY') { throw "Launcher did not fail closed for flags: $flags; exit=$LASTEXITCODE; output=$output" }
    if (Test-Path -LiteralPath $root) { throw 'Launcher created data root' }
  }
  New-Item -ItemType Directory -Path $root | Out-Null
  $marker = Join-Path $root 'foundation.sqlite'
  [IO.File]::WriteAllText($marker, 'legacy database sentinel')
  $before = (Get-FileHash -LiteralPath $marker -Algorithm SHA256).Hash
  $output = & pwsh -NoProfile -File $launcher -DataRoot $root -AllowDegraded -NoProbe 2>&1 | Out-String
  if ($LASTEXITCODE -eq 0 -or (Get-FileHash -LiteralPath $marker -Algorithm SHA256).Hash -ne $before) { throw 'Existing root changed or launcher succeeded' }
  if (@(Get-ChildItem -Force -LiteralPath $root).Count -ne 1) { throw 'Launcher created files in existing root' }
  Write-Output 'PASS: five flag combinations and existing data root fail closed'
} finally {
  if ((Split-Path -Parent $case) -ne $parent) { throw 'Unexpected test directory' }
  Remove-Item -LiteralPath $case -Recurse -Force
}



