# Windows 原生编译环境诊断

按用户确认的部署边界，FlashHead 优先使用 Windows 环境，Qwen 保留 WSL 中的现有 API 服务；本轮没有调整 Qwen 或替换既有 PyTorch。

## 已核实

- Windows Python 3.10、PyTorch 2.7.1+cu128 可识别 RTX 5090 Laptop GPU；本机 CUDA Toolkit 13.1 可提供编译所需库。CUDA Toolkit 版本与 PyTorch 自带运行时版本不同，不表示已验证所有 CUDA 算子兼容。
- 在 E 盘隔离目录安装 Triton Windows 3.3.1.post21，wheel SHA-256 为 `ef25f3ec1893a16af3dafca6d148fce3ea78f36eeb8ea08108ece56726d64c0b`。此为社区 Windows 版本，非 PyTorch 官方发行物，正式交付前仍需冻结依赖与许可。
- 在指定 CUDA 库和 E 盘缓存后，小矩阵 torch.compile 真实 GPU 运算通过，结果有限且最大绝对误差为 0；冷运行 5.519 秒。证据：`target-windows/flashhead-acceptance/windows-triton331-compile-probe.json`。
- 诊断脚本新增 `--require-compile`，检查 PyTorch 全局禁用状态和模型是否被包装，防止将关闭编译的 eager 结果误判为编译结果；包装本身不代表实际图已成功运行。

## 未完成与失败记录

- 首次真实模型编译遇到 Triton 缓存文件路径长度限制；改为 `target-windows/t` 短缓存后越过该错误。
- 第二次实际模型确认编译未禁用且模型为 `OptimizedModule`，预热 ready 约 47.9 秒；首块累计 611.047 秒生成 24 帧有限值，随后在块边界因超过 600 秒期限失败。中止时报告文件尚未刷新，先前误认为没有出帧；现已依据最终 JSONL 更正。日志：`target-windows/flashhead-acceptance/windows-triton-face512-shortpath.jsonl` 与同名前缀 stderr 日志。单块含编译耗时，不能判断稳态性能或口型质量已改善。
- `TORCH_COMPILE_DISABLE=0` 在当前 PyTorch 中仍是非空值，会禁用编译；有效测试必须移除该环境变量。此前对应两块输出为 eager，不作编译性能证据。
- Windows 完整模型编译和正确性、Qwen 联合负载、真实播放/OBS 的 10 分钟 M0 与嘴部清晰度均未闭环；不更改验收状态。

下一步定位首图编译瓶颈并研究可测量的分段或有限编译；先建立冷启动、热启动、稳定出帧和画质对照，再决定是否纳入正式 worker。所有新缓存保持在 E 盘。
