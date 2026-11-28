# 768 编译路径热点

当前 SageAttention 2.2／768／49 帧／四步，模型与 VAE 均使用 torch.compile，同时开启 cuDNN benchmark。授权素材和已有语音，第二块采集 CPU/CUDA；无公共窗口、Qwen 合成或 OBS。该配置与此前无编译轮次还存在 cuDNN 搜索差异，不是单变量编译加速实验。

有效 trace `target-windows/flashhead-acceptance/torchprofile768-compiled-20261106.trace.json`：6237 个 GPU kernel，累计 1371.46 ms；前三类卷积／CUTLASS 矩阵乘累计 434.18、358.03、185.40 ms，合计约 71.3%。SageAttention 主要核约 54.46 ms，复数乘主要核约 32.0 ms，gpu_memcpy 累计约 0.05 ms。CPU 算子嵌套和 GPU 事件累计均不直接等于关键路径墙钟，采样未覆盖跨进程输出的复制和 OBS。

第二块采样墙钟约 1.8895 秒，包含 profiler 开销。首块约 28.98 秒（含预热），不能并入稳定性能。原始 JSONL 同前缀。未测得新加速，也不授予实时通过。

下一优化任务应围绕主要卷积／GEMM 的后端实现做一次固定形状、固定精度、充分预热的受控比较，保持现有质量基准。先检查目标硬件原生内核支持和版本组合，再隔离测试，禁止直接升级覆盖当前可运行环境；已有 TensorRT FFN 候选继续停止扩展。若此项不能给实际链路至少 15% 改善，则按复审方案转向单独规格的区域生成或已验证外部算力。剩余编译 512 对照与 Qwen 争用仍未完成，不将 0.69 勾选。
