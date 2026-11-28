# V5.6.1 依赖探测与启动器实施记录

日期：2026-11-04。任务：1.3a、1.4。状态：启动器/探测已实现；安装包与模型下载器未完成；目标机 Windows M0 未执行。

## 实现

| 文件 | 内容 |
|---|---|
| `crates/nai-runtime-core/src/v561_probe.rs` | 依赖探测：数据根/config/Qwen/FlashHead/Python/PyTorch/CUDA/GPU/ffmpeg/OBS/音频 |
| `apps/workbench-engine/src/bin/v561_probe.rs` | `nai-v561-probe --data-root <path> [--strict]`，输出 JSON |
| `scripts/v561-start.ps1` | V5.6.1 启动器：构建/检查/探测/启动桌面，不回退 V4.8 engine |
| `apps/workbench-engine/Cargo.toml` | 新增 `nai-v561-probe` bin |
| `adapters/storage-sqlite/src/v561.rs` | 默认配置增加 `avatar.model_dir`、`avatar.worker_python`、`output.obs_path/audio_device` |

## 探测能力

| 检查 | 行为 |
|---|---|
| 数据根 | 检查 `assets/logs/recordings`，缺失为必填失败 |
| config | 检查可读/对象；`avatar.engine != flashhead` 为 warn |
| Qwen3-TTS | 仅允许 loopback HTTP；2xx=ok，其他 HTTP=warn，不可达=missing（必填） |
| FlashHead | 检查 `avatar.model_dir` 存在且非空；缺目录=missing（必填） |
| Python | 执行 `python --version`；不可执行为 missing |
| PyTorch/CUDA | 执行 `python -c "import torch..."`；CUDA false 为 warn |
| GPU | `nvidia-smi` 查询名称/驱动/显存；不可用=missing |
| ffmpeg | `ffmpeg -version`；不可用=missing |
| OBS | `output.obs_path` 存在=ok，否则 warn；不把进程存在当已捕获 |
| 音频 | 设备枚举未接入，显式 unknown；不伪造 ok |

安全边界：

- HTTP 只允许 `127.0.0.1/localhost/[::1]` 的 `http://`，不访问公网/HTTPS。
- 命令带超时（Python import torch 20s），超时 kill 并报告。
- 不读取/打印 API Key；不启动模型、不打开音频设备。

## 本机探测结果（Linux/WSL，非 Windows M0）

```bash
cargo run -q -p nai-workbench-engine --bin nai-v561-probe -- --data-root /tmp/nai-v561-probe
```

实际结果摘要：

- data-root/config：ok
- Qwen：`http://127.0.0.1:8000/health` → HTTP 200；模型 `qwen3-tts-1.7b-customvoice`，24k，非流式（后续实测见 `2026-11-04-qwen3-tts-capability.md`）
- FlashHead：缺少 `assets/models/flashhead` → missing（必填，整体 blocked）
- Python：3.12.3 → ok
- PyTorch：系统 Python 导入失败 → missing（需 worker venv）
- GPU：RTX 5090 Laptop GPU, 24463 MiB → ok
- ffmpeg：6.1.1 → ok
- OBS：未配置 `output.obs_path` → warn
- audio：unknown
- overall：blocked

这不是 M0 通过；只证明探测能区分 ok/warn/missing/unknown，且不会把缺失写成通过。

## 启动器

```powershell
pwsh scripts/v561-start.ps1 `
  -DataRoot E:\nai-data `
  -RuntimeDir E:\projects\nai\target-windows\v561-runtime
```

行为：

- `-Build` 可选构建 engine/keeper/probe；
- 检查 ffmpeg/nvidia-smi/OBS；
- 运行 probe；`overall=blocked` 默认拒绝启动，`-AllowDegraded` 可进入降级；
- 启动 desktop，设置 `NAI_WORKBENCH_TEST_ROOT`、`NAI_FOUNDATION_ENGINE`、`NAI_PUBLIC_OUTPUT_KEEPER`；不设置旧 engine；
- 不自动下载模型/不自动安装 OBS。

## 测试

```bash
cargo test -p nai-runtime-core v561_probe
cargo check -p nai-workbench-engine --bin nai-v561-probe
```

结果：3 项探测测试通过；probe bin 可构建并可运行。

## 未完成

- 1.3b：安装包、avatar-worker venv 自动安装、模型下载/哈希校验。模型权重来源/许可未冻结，先不自动下载。
- 音频设备枚举/默认设备丢失检测（CPAL/WASAPI 接线）。
- OBS 进程/窗口/捕获/音轨/信号分层检测（当前仅 `obs_path` + 进程提示）。
- Windows 目标机 M0（需 Windows 11 + 5090 Laptop；当前探测是 Linux/WSL，不可当作 M0）。
- Qwen 已确认：`/health` 200、1.7B CustomVoice、24k WAV、非流式；见 `2026-11-04-qwen3-tts-capability.md`。

## 下一步

1. 1.3b：模型清单/venv/安装器，前提是许可与权重来源确认。
2. 1.5：配置读写/脱敏/系统凭据。
3. 2.x：Qwen3-TTS 适配与流式探测。
4. 3.x：FlashHead worker 协议与 Lite 集成。
