# 2026-11-11 v561 形象模糊根因与 512/3 步优化（ACC 轨）

变更：`openspec/changes/v561-avatar-512-three-step`。前置：512/2 受控播放（同日归档）、
用户实测视频模糊。决策门：ADR 0203；34ACC 状态不变；失败账本保留。

## 模糊根因（证据链）

1. **人眼证据**：用户实测 512/2 二进制输出模糊。原生分辨率脸部裁剪抽看帧证实——
   512/2 面部皮肤纹理/胡茬/唇须明显丢失（塑料感）；512/4 与 768/4 细节在位。
   矩阵阶段"轻微柔化"的评级系整图下采样查看 + 整图 Laplacian 指标被头发对比度误导，
   指标本身不可作质量通过（已如实记录两次指标与观感背离）。
2. **代码证据**：上游 `flash_head/src/pipeline/flash_head_pipeline.py:166-172`——
   lite 检查点官方默认 4 步（[1000,750,500,250]），2 步 [1000,500] 为受支持但非蒸馏
   主目标；少两个低噪声步直接损失细节。本实现的 2 步覆盖与上游逐值一致，排除实现偏差。
3. **排除项**：512 分辨率本身不背锅（512/4 细节完好）；直播流式路径不背锅
   （同配置批式抽看帧即现柔化）。

## 优化实验（实验轨，target-windows/v561-timesteps-3step）

同 PCM/种子/编译边界（compile+eager-sdpa+compile-blocks），96 帧采集：

| 时间表 | compiled-warm | 脸部抽看（048/095 原生裁剪） | 结论 |
| --- | ---: | --- | --- |
| [1000,500]（2 步） | 2297ms（含 IO） | 塑料感，细节丢失 | 降级为诊断用途 |
| **[1000,500,250]（3 步）** | **1812ms（含 IO）** | **≈4 步：胡茬/皮肤纹理在位** | **采纳** |
| [1000,500,1]（linspace 通式） | 1875ms（含 IO） | 仍偏软 | 拒绝 |
| [1000,750,500,250]（4 步） | 3015ms（含 IO） | 参照（质量良好） | 质量退路 |

3 步为 4 步网格子集（仅跳 750），RTF 0.47（含采集 IO），低于 2 步同口径 2297ms
（不同轮次单次运行，IO 口径已标注；相对结论以质量抽看为准）。
实现：诊断用 `override_timesteps`（显式列表、上游同顺序、生产配置面不接受）、
探针 `--timesteps`，2 项 CPU 测试（共 8 项通过）。实验首轮 cmd 因 `%errorlevel%`
块内展开缺陷漏判失败，已改 `if errorlevel 1` 并重跑，账本保留。

## 变更内容（B01–B03）

- Rust `Config::validate` 接受 2/3/4；测试更名 `sample_steps_only_two_through_four_and_roundtrip`，
  合法值 [2,3,4]、拒绝值 [0,1,5,8]；nai-workbench-engine 77 过 + 2 既有环境门控。
- Python `config_sample_steps`/`override_sample_steps` 支持 3（`_SCHEDULES` 映射
  [1000,500,250]，同一覆盖点、同一顺序）；8 项 CPU + 4 项相邻测试通过；
  探针 `--sample-steps` 扩到 {2,3,4}。
- 验收运行目录配置切 512/3；v561_live_control.exe 重建并核验 mtime 新于源码
  （DrvFs mtime 缓存教训）；canonical 配置与用户运行版不动。

## 验收账本（B04）

| 尝试 | 结果 | 说明 |
| --- | --- | --- |
| 07 | PREPARE_FAILED | 加载期 0xC0000005（可用提交内存 15.1GiB，边界抖动） |
| 08 | PREPARE_FAILED | 同上（16.0GiB，无残留进程/GPU 占用） |
| 09 | PREPARE_FAILED | 同上（16.0GiB） |
| 10 | PREPARE_FAILED | 同上（14.8GiB；重启 CosyVoice 容器后 Windows 侧回收缓慢，一度降至 10.1GiB） |
| 11 | 三句完成；1 项断言红 | 回收至 19.1GiB 后通过加载；结果见下 |

内存教训补充：加载成败边界实测约 ≥16GiB 可用提交内存且需等待 Windows 回收稳定；
重启 WSL 内大容器短期反而降低可用量。

## 512/3 原生三句验收（attempt 11，script 单调时钟 ms）

- 三句全部完成、`model_api_calls=3`（本地 TTS、0 费用）、单窗口、单设备、
  worker/device pid 恒定、各句 `played_samples == samples`、epoch=3。
- 唯一红断言 `ready_before_previous_device_end_count=0`（既有 gapless 前沿，
  canonical 768/4 与 512/2 同红，非本变更回归；断言未放宽）。
- 句间 gap 1459/984ms（本句 TTS 切分长度与 512/2 轮不同，数值不直接可比；
  两前轮 2615–3934 → 1196/1240 → 本轮同量级）。
- 首声 7668ms；source_mode streaming_first/future/future、video incremental_gpu_stream、
  增量消费（10812<11750、14330<15098）、GPU 预取重叠（8296<10393、11751<14217）全部满足。
- ready 缺口：source_ready 超 device_end 约 750/140ms（句 2/句 3），缺口较 512/2 收窄。

## 下一步

- 512/3 分钟级口型/身份稳定性未验证（HEAD-01 后续）；用户实测仍有异议时退路为 512/4
  （仅改配置字段）。
- gapless 前沿不变：设备队列容量上限实验（96000→144000 样本）仍为下一候选。
- 抽看帧以批式路径产物为准；流式路径帧未单独采集（如实记录）。
