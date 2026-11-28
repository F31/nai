#requires -Version 7.0
<#
.SYNOPSIS
  V5.6.1 workbench entry point.
.DESCRIPTION
  The V5.6.1 engine and desktop assembly is not yet available. This entry
  point fails closed rather than running the legacy foundation engine against
  the new data root. Dependency diagnostics can be run separately with
  nai-v561-probe --data-root <path>; the probe initializes a new data root.
#>
param(
  [string]$DataRoot = "",
  [string]$RuntimeDir = "",
  [switch]$AllowDegraded,
  [switch]$Build,
  [switch]$NoProbe
)

$ErrorActionPreference = 'Stop'
throw 'NAI_V561_ASSEMBLY_NOT_READY: V5.6.1 工作台尚不可启动：新引擎和桌面程序尚未接入 V5.6.1 数据根及协议。未启动旧引擎，也未修改数据；请单独运行 nai-v561-probe 进行依赖探测。'

