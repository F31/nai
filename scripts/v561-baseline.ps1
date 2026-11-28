#requires -Version 7.0
<#
.SYNOPSIS
  V5.6.1 target-machine baseline sampler.

.DESCRIPTION
  Samples GPU / VRAM / system RAM / temperature / power / FPS-related process data
  while the operator runs FlashHead Lite + Qwen3-TTS + OBS for a bounded window.
  It does not launch models or start a live session by itself.

  Output:
    samples.jsonl   one JSON object per sample
    summary.json    min/max/avg and environment snapshot
    state.json      Qwen/OBS/process observations

.PARAMETER OutputDir
  Output directory. Default: docs/reports/v561-baseline/<timestamp>.

.PARAMETER DurationSeconds
  Sampling duration. Default 600 (10 minutes). M1 should use 1800.

.PARAMETER MetricsPath
  Optional engine metrics JSON with fields such as first_audio_ms, first_frame_ms,
  fps, dropped_frames, av_sync_ms. Unknown values stay null, never zero.

.PARAMETER QwenEndpoint
  Qwen3-TTS health endpoint. Default http://127.0.0.1:8000/.

.PARAMETER ProcessName
  Process(es) to sample. Default nai-foundation-engine,nai-public-output-keeper.

.EXAMPLE
  pwsh scripts/v561-baseline.ps1 -DurationSeconds 600 -MetricsPath .\target\v561-metrics.json
#>
param(
  [string]$OutputDir = "",
  [ValidateRange(10, 86400)][int]$DurationSeconds = 600,
  [string]$MetricsPath = "",
  [string]$QwenEndpoint = "http://127.0.0.1:8000/",
  [string[]]$ProcessName = @("nai-foundation-engine", "nai-public-output-keeper")
)

$ErrorActionPreference = "Stop"
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
if ([string]::IsNullOrWhiteSpace($OutputDir)) {
  $OutputDir = Join-Path $PSScriptRoot "..\docs\reports\v561-baseline\$timestamp"
}
$OutputDir = [System.IO.Path]::GetFullPath($OutputDir)
if (Test-Path -LiteralPath $OutputDir) {
  if (!(Test-Path -LiteralPath $OutputDir -PathType Container)) { throw "Output path is not a directory: $OutputDir" }
  if (@(Get-ChildItem -LiteralPath $OutputDir -Force).Count -gt 0) { throw "NAI_BASELINE_OUTPUT_NOT_EMPTY: $OutputDir" }
} else {
  New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

function Get-NvidiaSmiPath {
  $cmd = Get-Command nvidia-smi -ErrorAction SilentlyContinue
  if ($null -eq $cmd) { return $null }
  return $cmd.Source
}

function Get-GpuSample([string]$NvidiaSmi) {
  if ([string]::IsNullOrWhiteSpace($NvidiaSmi)) { return $null }
  $query = "name,driver_version,memory.total,memory.used,temperature.gpu,power.draw,power.limit,utilization.gpu,clocks.current.sm"
  $raw = & $NvidiaSmi "--query-gpu=$query" "--format=csv,noheader,nounits" 2>$null
  if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($raw)) { return $null }
  $line = ($raw | Select-Object -First 1).Trim()
  $parts = $line -split "\s*,\s*"
  if ($parts.Count -lt 9) { return $null }
  function To-NumberOrNull([string]$Value) {
    $parsed = 0.0
    if ([double]::TryParse($Value, [System.Globalization.NumberStyles]::Float, [System.Globalization.CultureInfo]::InvariantCulture, [ref]$parsed)) {
      return $parsed
    }
    return $null
  }
  [pscustomobject]@{
    name = $parts[0]
    driver_version = $parts[1]
    vram_total_mb = To-NumberOrNull $parts[2]
    vram_used_mb = To-NumberOrNull $parts[3]
    temperature_c = To-NumberOrNull $parts[4]
    power_draw_w = To-NumberOrNull $parts[5]
    power_limit_w = To-NumberOrNull $parts[6]
    gpu_util_percent = To-NumberOrNull $parts[7]
    sm_clock_mhz = To-NumberOrNull $parts[8]
  }
}

function Get-MemorySample {
  $os = Get-CimInstance Win32_OperatingSystem
  $totalMb = [math]::Round($os.TotalVisibleMemorySize / 1024, 1)
  $freeMb = [math]::Round($os.FreePhysicalMemory / 1024, 1)
  [pscustomobject]@{
    ram_total_mb = $totalMb
    ram_free_mb = $freeMb
    ram_used_mb = [math]::Round($totalMb - $freeMb, 1)
  }
}

function Get-ProcessSample([string[]]$Names) {
  $items = @()
  foreach ($name in $Names) {
    $procs = Get-Process -Name $name -ErrorAction SilentlyContinue
    foreach ($p in $procs) {
      $items += [pscustomobject]@{
        name = $p.ProcessName
        pid = $p.Id
        working_set_mb = [math]::Round($p.WorkingSet64 / 1MB, 1)
        cpu_seconds = if ($null -ne $p.CPU) { [math]::Round($p.CPU, 1) } else { $null }
      }
    }
  }
  return $items
}

function Get-QwenState([string]$Endpoint) {
  try {
    $response = Invoke-WebRequest -Uri $Endpoint -Method Get -TimeoutSec 5 -UseBasicParsing
    return [pscustomobject]@{ reachable = $true; status = [int]$response.StatusCode; error = $null }
  } catch {
    return [pscustomobject]@{ reachable = $false; status = $null; error = $_.Exception.Message }
  }
}

$nvidiaSmi = Get-NvidiaSmiPath
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$os = Get-CimInstance Win32_OperatingSystem
$environment = [pscustomobject]@{
  timestamp = (Get-Date).ToString("o")
  host = $env:COMPUTERNAME
  os = $os.Caption
  os_version = $os.Version
  cpu = $cpu.Name
  logical_processors = $cpu.NumberOfLogicalProcessors
  powershell = $PSVersionTable.PSVersion.ToString()
  nvidia_smi = $nvidiaSmi
  qwen_endpoint = $QwenEndpoint
  duration_seconds = $DurationSeconds
  processes = $ProcessName
}

$qwen = Get-QwenState $QwenEndpoint
$obs = Get-Process -Name "obs64", "obs32" -ErrorAction SilentlyContinue | Select-Object -First 1
$state = [pscustomobject]@{
  timestamp = (Get-Date).ToString("o")
  qwen = $qwen
  obs_running = ($null -ne $obs)
  obs_pid = if ($null -ne $obs) { $obs.Id } else { $null }
  metrics_path = $MetricsPath
}
$state | ConvertTo-Json -Depth 6 | Set-Content -Path (Join-Path $OutputDir "state.json") -Encoding utf8

$samples = New-Object System.Collections.Generic.List[object]
$stopwatch = [Diagnostics.Stopwatch]::StartNew()
$wallStart = Get-Date
$previousWall = $null
$maxSampleGapMs = $null
$intervalMs = 2000
while ($stopwatch.Elapsed.TotalSeconds -lt $DurationSeconds) {
  $sampleWall = Get-Date
  if ($null -ne $previousWall) {
    $gapMs = ($sampleWall - $previousWall).TotalMilliseconds
    if ($null -eq $maxSampleGapMs -or $gapMs -gt $maxSampleGapMs) { $maxSampleGapMs = $gapMs }
  }
  $previousWall = $sampleWall
  $gpu = Get-GpuSample $nvidiaSmi
  $mem = Get-MemorySample
  $procs = Get-ProcessSample $ProcessName
  $sample = [pscustomobject]@{
    timestamp = $sampleWall.ToString("o")
    elapsed_ms = [math]::Round($stopwatch.Elapsed.TotalMilliseconds)
    gpu = $gpu
    memory = $mem
    processes = $procs
  }
  $samples.Add($sample)
  ($sample | ConvertTo-Json -Depth 6 -Compress) | Add-Content -Path (Join-Path $OutputDir "samples.jsonl") -Encoding utf8
  Start-Sleep -Milliseconds $intervalMs
}

$stopwatch.Stop()
$wallElapsedMs = ((Get-Date) - $wallStart).TotalMilliseconds

$gpuSamples = @($samples | Where-Object { $null -ne $_.gpu })
$memorySamples = @($samples | Where-Object { $null -ne $_.memory })
function Measure-Field($Items, [string]$Path) {
  $values = @()
  foreach ($item in $Items) {
    $value = $item
    foreach ($segment in $Path.Split('.')) {
      if ($null -eq $value) { break }
      $value = $value.$segment
    }
    if ($null -ne $value) { $values += [double]$value }
  }
  if ($values.Count -eq 0) { return $null }
  $measure = $values | Measure-Object -Minimum -Maximum -Average
  return [pscustomobject]@{
    min = [math]::Round($measure.Minimum, 2)
    max = [math]::Round($measure.Maximum, 2)
    avg = [math]::Round($measure.Average, 2)
    samples = $values.Count
  }
}

$metrics = $null
if (-not [string]::IsNullOrWhiteSpace($MetricsPath) -and (Test-Path -LiteralPath $MetricsPath)) {
  try { $metrics = Get-Content -LiteralPath $MetricsPath -Raw | ConvertFrom-Json } catch { $metrics = @{ error = $_.Exception.Message } }
}

$summary = [pscustomobject]@{
  baseline = "V5.6.1"
  environment = $environment
  state = $state
  sample_count = $samples.Count
  elapsed_ms = [math]::Round($stopwatch.Elapsed.TotalMilliseconds)
  wall_elapsed_ms = [math]::Round($wallElapsedMs)
  max_sample_gap_ms = if ($null -ne $maxSampleGapMs) { [math]::Round($maxSampleGapMs) } else { $null }
  gpu = [pscustomobject]@{
    vram_used_mb = Measure-Field $gpuSamples "gpu.vram_used_mb"
    temperature_c = Measure-Field $gpuSamples "gpu.temperature_c"
    power_draw_w = Measure-Field $gpuSamples "gpu.power_draw_w"
    gpu_util_percent = Measure-Field $gpuSamples "gpu.gpu_util_percent"
    sm_clock_mhz = Measure-Field $gpuSamples "gpu.sm_clock_mhz"
  }
  system_ram = [pscustomobject]@{
    used_mb = Measure-Field $memorySamples "memory.ram_used_mb"
    free_mb = Measure-Field $memorySamples "memory.ram_free_mb"
  }
  engine_metrics = $metrics
  notes = @(
    "Unknown values stay null; do not replace them with zero.",
    "Run with FlashHead Lite + Qwen3-TTS + OBS already started by the operator.",
    "M0 requires 10 minutes; M1 requires 30 minutes; M2 requires 2 hours.",
    "Elapsed time and sample gaps are evidence only; script completion is not acceptance."
  )
}
$summary | ConvertTo-Json -Depth 10 | Set-Content -Path (Join-Path $OutputDir "summary.json") -Encoding utf8

Write-Output "V5.6.1 baseline samples: $($samples.Count)"
Write-Output "Output: $OutputDir"
if ($null -eq $nvidiaSmi) {
  Write-Error "nvidia-smi not found; GPU samples are unavailable."
  exit 2
}
exit 0
