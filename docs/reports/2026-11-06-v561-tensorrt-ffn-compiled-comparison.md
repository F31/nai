# V5.6.1 FFN 编译路径对照及止损

使用授权参考图与已有 Qwen 音频的真实 FlashHead 首层输入 `[1,4032,1536]` bf16，比较同进程 PyTorch eager、`torch.compile`、TensorRT 2.7（禁用 TF32、启用 FP32 累加、512 MiB workspace、优化等级 0）。三者先分别预热 3 次，再同步计时 5 次。TensorRT 确认实际生成 `TorchTensorRTModule`；此为一层 FFN，不是整个网络。

| 路径 | 五次耗时（毫秒） |
| --- | --- |
| PyTorch eager | 4.902、5.150、4.737、4.557、4.722 |
| PyTorch compile | 4.991、9.599、4.529、4.725、4.381 |
| TensorRT FP32 累加 | 12.124、12.398、17.822、13.723、14.676 |

对 eager 参考，TensorRT 最大绝对差 0.5、平均约 0.003952，差值超过 0.01 的元素 596379；PyTorch compile 最大绝对差同为 0.5、平均约 0.005362，参考最大绝对值 125.5。两者均可能因 bf16 算子融合等带来数值差异，但本次没有逐算子证明具体来源；不能单独归因 TensorRT，也没有对完整画质设定通过结论。TensorRT 构建约 46.7 秒。固定运行次序、样本较少、GPU 频率未控制，数字不构成完整模型基准。

此候选没有展示收益，并存在此前整网 dry-run 的 154 子图与跨框架切换成本；根据本轮止损条件，停止扩大此版本 TensorRT 分区实验，不修改默认。不是证明所有 TensorRT 优化或其他设备都无效。原始记录 `target-windows/flashhead-acceptance/trt-flashhead-ffn-fp32acc-20261106.jsonl`。M0、实数 RoPE 完整画质、衣物漂移、Qwen／OBS 联合实时仍未闭环；应转向更有希望的端到端推理方案评估，并保持 V5.6.1 现行基线，架构变更须另立规格。
