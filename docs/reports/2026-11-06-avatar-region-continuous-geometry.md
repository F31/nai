# 连续区域几何诊断

使用已授权视频的 25 FPS 副本，自第 250 帧顺序解码 100 帧（10–13.96 秒），现有 MediaPipe FaceMesh 独立识别。100 帧均单人脸，正方形区域全在画面内、嘴部四个语义点均在区域内；相邻帧区域中心最大位移为前一帧区域宽度的 0.02887。检测模式仍逐帧 static_image_mode，不称作已经实现时序跟踪或全片稳定。

诊断记录增加原图 roi_xyxy、roi_in_bounds 和 mouth_roi_normalized，明确区别于 FeatherTalk 特定编号的 .lms；未生成伪映射。原始 JSON 及预览位于 target-windows/avatar-region-research/dataset-girl/geometry-continuous。可视复核嘴部落点合理；字幕仍在底片内。此区域取完整脸，不代表最终嘴部训练裁剪或融合遮罩已确定。

下一步以语义几何建立候选专用训练／推理共享的嘴部裁剪和遮罩适配，必须验证原图到局部再回贴的同源重建、边界与遮挡；未训练、未输出新口型或证明实时。默认权重许可核查、模型质量、全片预处理和在线链路尚未完成。
