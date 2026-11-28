# V5.6.1 许可证门

日期：2026-11-04。状态：执行中。适用范围：V5.6.1 单引擎基线的开发、打包、分发与商用声明。

## 结论

- **FlashHead 不能简写为“整栈 Apache-2.0”**：代码仓库是 Apache-2.0，但 Hugging Face 模型卡的 license 为 `other`，并明确 VAE_LTX 为 OpenRAIL-M，不能简单再许可为 Apache-2.0。
- **LiveAct 未书面确认前锁定**：仓库无 LICENSE，模型卡条件不能替代书面确认；不打包、不分发、不进入主线。
- **Qwen3-TTS / EchoMimicV3 / OBS / ffmpeg 分别核查**：代码、权重、依赖、示例素材分开记录；不能因代码许可推定权重/素材许可。
- **用户素材另算**：肖像、参考音频/声音克隆、音乐/BGM、驱动视频必须有用户确认与来源记录；勾选不是法律证明。

## 许可矩阵

| 组件 | 代码许可 | 权重/模型许可 | 依赖/素材 | 当前状态 | 处置 |
|---|---|---|---|---|---|
| SoulX-FlashHead | Apache-2.0 | HF 模型卡 `license: other`；VAE_LTX OpenRAIL-M | 需列 PyTorch/FlashAttention/SageAttention 等 | 需复核 | 保留代码；权重分发/商用前书面确认；UI 显示限制 |
| SoulX-LiveAct | 仓库无 LICENSE | 模型卡待复核 | 18B/FP4/offload 依赖 | **Blocked** | 不打包/不分发/不进主线；仅隔离研究 |
| Qwen3-TTS 0.6B CustomVoice | 官方仓库 Apache-2.0 | 官方模型卡 Apache-2.0 | 参考音频/声线及运行依赖 | 部分核查 | 固定版本并核验权重；依赖、NOTICE 和分发边界仍需核对 |
| EchoMimicV3 | Apache-2.0 | 待复核 | 生成片段/肖像 | 需复核 | 仅作离线片段工具，不阻塞主线 |
| OBS Studio | GPL-2.0（需复核版本） | 不适用 | 外部软件 | 需复核 | 仅外部依赖，不打包进 nai |
| ffmpeg | LGPL/GPL（取决于构建） | 不适用 | 编解码 | 需复核 | 记录构建选项；不静态链接未确认组件 |
| 用户肖像/视频 | 用户提供 | 用户负责 | 授权范围/期限 | 未确认 | 导入时勾选/记录；未确认拒绝直播 |
| 用户声音/参考音频 | 用户提供 | 用户负责 | 说话人同意 | 未确认 | 未确认不使用克隆 |
| 音乐/BGM | 用户提供 | 用户负责 | 直播/录制权 | 未确认 | 未确认不播放/不录制 |

## 发布门

生成 Release Manifest 前必须满足：

1. 每个组件有 `code_license`、`weights_license`、`dependency_license`、`evidence`、`scope`。
2. `status=blocked` 的组件不得出现在发布包/安装器/默认路径。
3. FlashHead 权重/VAE 的再分发与商用条件有书面结论。
4. LiveAct 无书面许可时，安装器不下载、不打包、UI 不提供。
5. 用户素材许可在首次导入时记录；未确认不允许开始直播。
6. 发布报告列出未确认项与限制；不得用“代码 Apache”替代整栈结论。

## 证据要求

- 代码：仓库 LICENSE/NOTICE 文件与 commit。
- 权重：模型卡 license、附加条款、下载来源、哈希。
- 依赖：锁文件、wheel/包名、许可证清单（SBOM）。
- 用户素材：授权文本/来源/时间/范围；不存敏感原文到公开报告。
- 发布：`docs/reports/` 下机读清单 + 可读报告；未确认项明确 `blocked`。

## 当前阻塞

- 目标机资源基线未执行（需 Windows 11 + RTX 5090 Laptop 24GB + 32GB RAM）。
- FlashHead 权重/VAE 许可未取得书面结论。
- LiveAct 许可未确认。
- Qwen3-TTS 0.6B 官方代码及模型卡标注 Apache-2.0；依赖、NOTICE、分发边界和用户声音权利仍需核查。

**未满足以上条件前，V5.6.1 只能做内部开发/研究，不得声明可商用或已通过发布。**
