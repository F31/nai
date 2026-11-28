# 设计
通过诊断回执回调在精确释放前复制共享槽，拒绝错epoch/顺序/代次/摘要；每帧最大4MiB、clip最大256MiB，按帧写E盘全新目录，未完成保留partial。只有完整验证帧数并关闭文件后才形成完成产物。

使用相同PCM和全局/私有seed，baseline-warm与compiled-warm分别存像素。捕获IO会影响耗时，明确不能沿用本轮时长作纯性能基准。离线逐帧计算RGB MAE/RMSE/PSNR/最大差异和时间变化，不据PSNR直接认定口型质量；视频使用已有ffmpeg与原PCM精确时轴制作，不上传。人工质量未知时不激活生产或扩大Rust接线。
