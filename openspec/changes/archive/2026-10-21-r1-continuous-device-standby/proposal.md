## Why
常驻窗口已接，但句间黑屏、音频stream逐句创建，64MiB整句RGBA限制正常分辨率。需完成R1常驻音画的下一个可验证增量，才能接连续讲品。
## What Changes
会话音频stream复用、合法2D闭嘴静音待机。新增v2媒体描述符：逐帧摘要与独立待机摘要；帧文件分帧读取并复检，单句磁盘512MiB/20秒、单帧16MiB，旧v1保持64MiB闭包限制。状态报告stream打开次数与待机摘要作诊断，不产生硬件PlaybackAck。
## Impact
V3.11 §8.4/INV-21，registry A01/R01/R03/S12相关；engine、avatar-2d、keeper、私有Protobuf与直接消费者。无新增常驻依赖、无付费调用。不将连续讲品、真人麦克风或OBS录制标为完成。
