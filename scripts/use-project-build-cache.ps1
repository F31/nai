# Dot-source before Windows build/test commands. Runtime data and credentials are separate.
param([string]$Target = 'build')
$ErrorActionPreference = 'Stop'
if ($Target -notmatch '^[a-zA-Z0-9_-]+$') { throw 'Invalid project build target name' }
$naiProjectRoot = Split-Path -Parent $PSScriptRoot
$naiTargetRoot = Join-Path $naiProjectRoot 'target-windows'
$naiToolCache = Join-Path $naiTargetRoot 'tool-cache'
foreach ($folder in @('cargo','npm-cache','pnpm-store','pnpm-cache','pnpm-state','tmp')) {
    New-Item -ItemType Directory -Path (Join-Path $naiToolCache $folder) -Force | Out-Null
}
$env:CARGO_HOME = Join-Path $naiToolCache 'cargo'
$env:CARGO_TARGET_DIR = Join-Path $naiTargetRoot $Target
$env:NPM_CONFIG_CACHE = Join-Path $naiToolCache 'npm-cache'
$env:npm_config_store_dir = Join-Path $naiToolCache 'pnpm-store'
$env:npm_config_cache_dir = Join-Path $naiToolCache 'pnpm-cache'
$env:npm_config_state_dir = Join-Path $naiToolCache 'pnpm-state'
$env:TEMP = Join-Path $naiToolCache 'tmp'
$env:TMP = $env:TEMP
$env:TMPDIR = $env:TEMP
$naiNodeBin = Join-Path $naiToolCache 'windows-tools/node_modules/node-win-x64/bin'
if (Test-Path -LiteralPath $naiNodeBin -PathType Container) { $env:PATH = $naiNodeBin + ';' + $env:PATH }
$naiProtoc = Join-Path $naiToolCache 'windows-tools/protoc-21.12-win64/bin/protoc.exe'
if (Test-Path -LiteralPath $naiProtoc -PathType Leaf) { $env:PROTOC = $naiProtoc }
