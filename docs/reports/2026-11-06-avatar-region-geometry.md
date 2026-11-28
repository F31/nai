# 局部驱动稀疏几何核查

新增只读诊断脚本 avatar_region_geometry_probe.py，使用现有 MediaPipe 0.10.9 FaceMesh 对独立 25 FPS 素材抽取 8 个位置，不导出或伪造 FeatherTalk .lms。7 个样本成功解码且检测单人脸，最后一个位置随机定位解码失败，明确记 rejected_decode；不是全片损坏判断。可视检查成功样本的面框和嘴部四点落点合理，但尚未验证连续跟踪、遮挡、姿态门限或模型训练几何。

预览和 JSON 位于 target-windows/avatar-region-research/dataset-girl/geometry-review-v2；第一轮遇末尾失败即中止的结果保留在 geometry-review，后续脚本改为逐样本记录失败。预览确认原视频存在中英文烧录字幕；新语音样片须明确这一素材限制，正式输出需要无字幕授权素材或另行验证处理，不自动抹除。现有 MediaPipe 诊断不授予候选模型或全部依赖商用许可通过。

未训练、未生成新口型；下一步仍需定义候选专用裁剪／嘴部掩码语义，并在训练与推理中一致使用，而非映射假关键点编号。OpenSpec 素材准备仍未全部完成。
