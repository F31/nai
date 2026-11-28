# V5.6.1 退役方案清理与基线对齐

## 本轮交付

按用户要求删除 27 个 MuseTalk 专用研究脚本（含旧环境探测）、14 个专用 Python 测试、公共输出的 `research_clock_bridge` 示例及其专用 Cargo feature；删除独立 MuseTalk 源码副本、研究模型装配目录及 6 个旧模型下载文件。用户授权的视频/音频、历史报告、现有项目数据、凭据及可能复用的 PyTorch 环境和 wheel 保留。清理不删除 V4.8 只读历史注册表，不将其作为 V5.6.1 验收。

删除没有生产调用方的旧 workspace 形象授权原型；公共输出正在使用的同流音频组件仍有真实消费者，不能简单删除，后续随 V5.6.1 音视频链路替换。

对齐权威基线：新默认语音模型设为 `Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice`；此前 1.7B 服务仅保留为历史测量，0.6B 的服务模型 ID、性能和流式能力必须在目标部署上实测。现有用户配置不自动修改。缺配置时不再建议删除数据根。Qwen 健康返回成功只说明服务可达；FlashHead 目录非空不能当作模型完整/可用，直到清单和加载验证实现前保持缺失。

## 核查与测试

- OpenSpec v561 strict 通过；18 个 workspace 包依赖边界通过；V5.6.1 registry 检查通过，34 项 pending。
- Windows：V561 存储 10 项通过；Qwen 定向 5 项通过（其中 1 项 transport）；探测 5 项通过；公共输出和存储/engine/LLM 相关 cargo check 通过。存储测试最初 10/11：符号链接测试因 Windows 当前账户缺创建权限（1314）失败；后续单独跳过该项，不能声称 Windows 符号链接防逃逸验证通过。
- 沙箱内 Cargo 缓存视图缺少 `cap-fs-ext`，在受控环境补齐锁定依赖到 E 盘后运行测试。注册表检查使用 E 盘现成独立 Node 依赖运行相同脚本，因为根目录 YAML 链接不可解析；未更改共享 node_modules。
- 本机 Windows 检测 RTX 5090 Laptop 24GB 和 OBS 可执行文件；本机 Qwen health 当前不可达。另有 E 盘旧研究 venv PyTorch 2.7.1+cu128 报 CUDA 可用；并不代表 FlashHead 环境、权重或 M0 已通过。

## 尚未完成

`nai-foundation-engine` 仍直接调用旧 `Store`，存储、engine 和桌面 UI 仍含工作区/审批/预算等旧实现。需要新 V5.6.1 组合根和单数据根调用接替，再删除旧代码、旧协议及其依赖；本轮没有宣称全面完成。FlashHead 许可、权重/worker、M0、真实音画与 OBS 仍待完成。历史模型/性能结果仅供追溯，不计入 V5.6.1 验收。
