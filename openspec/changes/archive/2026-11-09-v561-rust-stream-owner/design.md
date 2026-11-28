# Design

本轮增加一次真实本地CosyVoice纵向接线：应用编排层调用现有适配器，规范PCM直接交Rust owner，完成回执通过后才提交EOS，错误取消临时任务，不自动重试。真实首PCM33.65秒未达性能目标；设备播放保持未装配，不把像素成功当直播完成。

唯一Rust owner装配共享私有spawn环境（仅原生工具链白名单，E盘缓存）、协议2签名和有界stdio。维护Ready独立<=240秒，业务<=45秒不放宽。raw只单槽、全连接payload ID单调；经过签名RenderCancelled且raw完整交付后才可退槽，损坏通道拒绝复用。Frame使用既有只读helper/Ingress；数量先限max_samples，再复检帧源起点不超实际交付PCM，EOS后精确count/digest/stats。任何协议异常关闭子进程，不冒充取消恢复。先Root operation取消再用独立4秒清理等待RenderCancelled；取消期间不释放或呈现迟到帧。只提交完整成功结果。
