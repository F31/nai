# 实验模型训练／评估模式诊断

使用隔离检查点，在训练帧和留出帧各抽2帧，分别渲染目标、遮蔽输入、eval输出、train模式输出；train模式诊断保存并恢复所有buffer，不保存修改后的检查点。eval输出近白且误差0.42–0.46，train模式输出恢复嘴部且误差0.019–0.031，说明此前高验证误差不能简单归因未见片段或模型无学习能力。也不能用train模式推理宣称修复，因为它依赖当前输入统计。

另仅用训练集64个均匀样本重估BatchNorm累计统计，4帧eval误差仍约0.094–0.441，未稳定修复；保留 model-review-calibrated。此试验未证明所有差异仅由BatchNorm导致，但模式敏感性明确。后续脚本以 --calibrate-bn 64 显式启用，该轮调用为参数化前实现，默认不再校准。

产物在 target-windows/avatar-region-research/dataset-girl/model-review、model-review-modes、model-review-calibrated。后续应将小批量归一化策略作为独立候选重训，例如不依赖运行均值的GroupNorm，与原结构同数据对照；涉及架构变化，不复用旧检查点兼容路径。必须同时验证嘴部清晰度、音频依赖和实际eval模式输出，再决定是否生成新语音样片。未经用户听感和动态评估，不授予质量或实时通过。
