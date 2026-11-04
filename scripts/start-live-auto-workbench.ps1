# Isolated engineering bundle; uses the normal data/credential scope, never test overrides.
$project = Split-Path -Parent $PSScriptRoot
& (Join-Path $PSScriptRoot 'start-preparation-workbench.ps1') -BinaryDirectory (Join-Path $project 'target-windows/live-auto-runtime')
