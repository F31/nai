# V5.6.1 FlashHead TensorRT 覆盖诊断

在 Windows RTX 5090 Laptop 上，以授权参考图、已有本地 Qwen 语音、768 构图、49 帧和 SageAttention 2.2 构造真实去噪输入；潜变量 `x/y` 是 `[1,128,7,24,24]` bf16，语音条件为 `[1,49,5,12,768]`。此轮为导出及 TensorRT dry-run，未播音、未推流、未改正式默认。

1. PyTorch 严格导出在 SageAttention 内的 `torch.cuda.device_count()` 失败，尚未进入 TensorRT 分区。
2. 非严格导出在 SageAttention 自定义 CUDA 内核访问 FakeTensor 数据指针时失败。
3. 仅在诊断进程中将 SageAttention 调用封装为不透明自定义算子，并提供假张量形状，严格导出成功（5330 个图节点）。随后 TensorRT 分区遇到 `torch.complex128` 不支持而失败；该诊断包装尚未做实际输出等价、完整数值或性能验证。

输入和失败日志分别在 `target-windows/flashhead-acceptance/trt-flashhead-coverage-20261106.*`、`trt-flashhead-nonstrict-20261106.*`、`trt-flashhead-opaque-20261106.*`。当前还没有 TensorRT 覆盖比例、可运行 FlashHead 引擎、加速或实时证据；不能将导出成功等同模型编译成功。后续需评估旋转位置编码复数运算可否以等价实数运算封装，并验证 SageAttention 混合分区的数值、切换开销和真实吞吐；任何改动需在完整质量和音画验证后才考虑默认启用。
