# V5.6.1 目标机基线状态

日期：2026-11-04。状态：**Blocked — 本机目标硬件已确认，但 FlashHead/Qwen/OBS 联合运行链路尚未就绪。**

本机短时检查已确认 Windows 11、RTX 5090 Laptop（24463 MiB）、约 32 GiB 内存；用户重启 WSL 后 Qwen 1.7B 已恢复并合成成功；FlashHead worker 与当前 1.7B 联合运行未验（0.6B 为可选优化）。见 `docs/reports/2026-11-04-v561-sampler-hardening.md`。

## 已完成

- 基线脚本：`scripts/v561-baseline.ps1`
- 采集项：GPU 名称/驱动/VRAM/温度/功耗/利用率/SM 时钟、系统 RAM、进程工作集、Qwen/OBS 状态、可选 engine metrics。
- 输出：`samples.jsonl`、`summary.json`、`state.json`。
- 未知值保持 `null`，不以 0 代替。

## 未完成（任务 0.6/0.7）

- 未在目标机运行 FlashHead Lite + Qwen3-TTS + OBS 10 分钟。
- 未采集首帧、首音、FPS、掉帧、AV 偏差的真实值。
- 未给出 Lite/Pro Go/No-Go。
- 未验证 30 分钟/2 小时/8 小时。

## 运行命令（目标机）

```powershell
# 先由操作员启动 Qwen3-TTS、FlashHead Lite worker、预览窗口和 OBS；
# 再运行采样脚本。
pwsh scripts/v561-baseline.ps1 `
  -DurationSeconds 600 `
  -MetricsPath .\target\v561-metrics.json `
  -QwenEndpoint http://127.0.0.1:8000/ `
  -OutputDir .\docs\reports\v561-baseline\m0
```

## M0 通过条件（建议）

| 指标 | 条件 |
|---|---|
| 时长 | 连续 10 分钟 |
| FPS | >=20（目标 25） |
| 首帧 | <=1.5s（目标，需实测校准） |
| 首音 | <=2.5s（目标，需实测校准） |
| VRAM | 无 OOM；记录峰值 |
| 系统 RAM | 无 OOM；记录峰值 |
| 温度 | <87°C；记录曲线 |
| 功耗 | 记录峰值与限频 |
| 停止 | <=200ms；旧帧/旧音频不续播 |

## Go/No-Go

- Lite 通过：允许进入 M1。
- Lite 失败：停止功能开发，先定位模型/驱动/Qwen/散热。
- Pro 通过：允许作为可选画质档；未通过只保留 Lite。
- 不允许用桌面 4090/5090 或双 5090 结果替代目标机。

## 证据

目标机运行后，将 `summary.json`、`samples.jsonl`、OBS 录像和本报告更新到 `docs/reports/`；未测项保持 `pending`/`blocked`。
