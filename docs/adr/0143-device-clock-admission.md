# ADR 0143：原生设备时钟准入边界

状态：Accepted（2026-10-21）。依据 V3.11 W05/W10 与 ADR0142，不改变完成 ACK 定义。

CPAL 0.16.0 WASAPI 实现读取 IAudioClock 时丢弃 position，只用 QPC 估计回调时间。新增 DeviceClockCursor 校验同一流身份、稳定非零频率、单调位置及已成功提交帧上界，使用 u128 整数换算后向下取整，不以 QPC、墙钟或声道值数量充当播放帧数。错误不更新游标。流重置/重建须重新取得零原点并创建新游标。

先以 Windows 显式 ignored 测试接入安全 wasapi 0.23.0（MIT），仅发送 48k 双声道浮点零样本。新依赖仅 Windows dev-dependency，不进入生产二进制，无新增常驻进程/端口/模型或 API 成本；项目代码继续 forbid unsafe。测试三秒总预算、100ms单次等待、最多4800帧缓冲，RAII停止流；真实位置推进及停止冻结分别验证。

本切片不替换现有常驻 CPAL 后端，不产生候选播放进度或 PlaybackAck，不自动推进节目，不表示可听声音、A/V 完成或 OBS 录制通过。下一轮将已验证接口接入常驻后端，并保持按候选/epoch/撤权/期限绑定，不能只把测试进度复制给生产消费者。

接口依据：[AudioClock](https://docs.rs/wasapi/0.23.0/wasapi/struct.AudioClock.html)、[Microsoft IAudioClock](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclock-getposition)。实际结果以本轮报告为准。
