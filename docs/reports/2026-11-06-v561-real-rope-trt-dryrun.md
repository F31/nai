# V5.6.1 实数 RoPE 与 TensorRT 分区诊断

隔离诊断以 float32 实部／虚部乘法代替原 float64／complex128 旋转位置编码，不修改上游文件或正式默认。RTX 5090 Laptop、固定随机种子的 1,548,288 个 bfloat16 输出中 16 个不同，最大绝对差 0.015625、平均绝对差约 4.31e-8，在该轮 `atol=0.03125, rtol=0.01` 检查内；这不是位一致，也不代表其他输入或最终视频质量通过。

在授权参考图、既有本地语音、768／49 帧及隔离 SageAttention 不透明封装下，真实去噪网络导出成功（6050 图节点），Torch-TensorRT dry-run 报告 4976 个算子中 4729 个受支持（95.04%），按最小分区大小 1 形成 154 个候选 TensorRT 子图；60 个 SageAttention 调用及 187 个 `_to_copy` 操作未转换。日志为 `target-windows/flashhead-acceptance/trt-flashhead-realrope-20261106.*`。

dry-run 未构建任何引擎，算子比例不等于耗时覆盖或加速比；154 个分区可能引入显著开销。实数 RoPE 对完整去噪结果及视频画质的影响、SageAttention 包装的真实等价性、实际 TensorRT 编译内存／延迟和 Qwen／OBS 联合吞吐均未验证。正式路径、默认精度及 M0 状态不变。后续先对分区大小与主要子图进行有界构建，记录实际引擎数量、编译成功率、显存、数值和速度，再判断是否值得继续整网方案。
