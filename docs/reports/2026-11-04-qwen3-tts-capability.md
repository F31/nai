# Qwen3-TTS 本地能力探测（V5.6.1）

日期：2026-11-04。服务：`http://127.0.0.1:8000/`。状态：已部署、已探测；采样/真实长稳仍需目标机 M0 记录。

## 服务事实

| 项 | 实测 |
|---|---|
| 健康 | `GET /health` → `{"status":"ok","attn":"sdpa","sample_rate":24000,"vram_gb":~3.9}` |
| 模型 | `qwen3-tts-1.7b-customvoice`；路径 `Qwen3-TTS-12Hz-1.7B-CustomVoice` |
| 声线 | `aiden, dylan, eric, ono_anna, ryan, serena, sohee, uncle_fu, vivian` |
| 语言 | `auto, chinese, english, french, german, italian, japanese, korean, portuguese, russian, spanish` |
| 接口 | `POST /v1/audio/speech`；`response_format=wav|mp3`；`speed` 暂不支持 |
| 音频 | 24 kHz、16-bit、单声道 WAV |
| 流式 | **不支持**；返回完整 WAV |
| 克隆 | 当前为 CustomVoice 预置声线；参考音频克隆/上传接口未在 OpenAPI 中出现 |

## 延迟实测（同机、curl 与 Rust probe）

| 场景 | 文本 | 音频时长 | 生成耗时 | RTF |
|---|---|---:|---:|---:|
| 冷启动/首句 | 你好，这是本地语音测试。 | 2.16s | 14.31s | 6.6× |
| 热态 1 | 欢迎来到直播间，今天我们介绍本地数字人方案。 | 4.32s | 6.85s | 1.59× |
| 热态 2 | 同上 | 4.08s | 4.21s | 1.03× |
| Rust probe 1 | 你好，这是本地 Qwen3-TTS 探测。 | 3.76s | 5.81s | 1.54× |
| Rust probe 2 | 同上 | 5.12s | 7.57s | 1.48× |

结论：**不是流式，首句可能 4–15s**；不能按“首包 150ms”设计。必须句级预取 + 缓存 + 短句优先，首句仍可能明显等待。

## 适配器

- 新增 `adapters/llm-openai/src/qwen.rs`：`discover`（health/models/voices/languages）、`synthesize`（复用 qwen_local TTS）、`probe`、`sentences`、`resample_i16`、`TtsCache`、`synthesize_cached`。
- 新增 `apps/workbench-engine/src/bin/v561_tts_probe.rs`，可重复探测并输出 JSON。
- 默认配置更新为：
  - endpoint `http://127.0.0.1:8000/v1`
  - model `qwen3-tts-1.7b-customvoice`
  - voice `vivian`
  - language `chinese`
  - stream `false`
  - timeout `45000ms`
- 依赖探测 `v561_probe` 改为检查 `/health`（从 `/v1` 派生），不再误报 404。

## 测试

```bash
cargo test -p nai-llm-openai qwen --locked          # 4 passed
cargo test -p nai-foundation-sqlite v561 --locked   # 11 passed
cargo run -q -p nai-workbench-engine --bin nai-v561-tts-probe -- --repeat 2
```

Rust probe 实际输出：`streaming=false`、`sample_rate=24000`、`error=null`、RTF 约 1.48–1.54。

## 设计影响

- 删除“流式优先”的默认承诺；改为“非流式 + 句级预取/缓存”。
- 首句等待是已知限制；直播开场应先播预置短句/缓存句，或提前合成。
- 1.7B 约 3.9GB VRAM，与 FlashHead Lite 预算兼容；但 24GB/32GB 仍需目标机 M0 记录峰值。
- 0.6B 仅作为后续可选优化，不再是 V5.6.1 默认。
- 真实声音听感/长文本/多声线/并发未验收；不得用本探测声称 M0 通过。

## 下一步

- 3.x：FlashHead worker 协议与 Lite 集成。
- 4.x：句级预取/播放接线、音频主时钟、停止/接管。
- 目标机 M0：同时记录 Qwen + FlashHead 的 VRAM/RAM/温度/FPS/首音。
