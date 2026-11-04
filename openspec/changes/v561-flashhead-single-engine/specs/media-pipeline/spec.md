# 媒体管线

## ADDED Requirements

### Requirement: 音频主时钟

系统 SHALL 以已播放音频样本作为媒体主时钟，按该时钟选择/推进视频帧；SHALL NOT 用墙钟、渲染完成时间或网络到达时间作为主时钟。

#### Scenario: 正常播放
- **WHEN** 音频设备播放语音
- **THEN** 视频帧按已播放样本位置选择，记录 AV 偏差

#### Scenario: 音频暂停
- **WHEN** 音频暂停或设备停止
- **THEN** 视频帧不继续按旧时钟前进；恢复时重新对齐

### Requirement: 共享内存媒体面

系统 SHALL 通过共享内存环形缓冲传输帧与 PCM，控制面只传引用/序号/时间戳；SHALL NOT 用 JSON/Base64 传输帧或 PCM。

#### Scenario: 出帧
- **WHEN** worker 生成一帧
- **THEN** 写入带 epoch/seq/pts/source_sample 的共享槽，engine 读取最新可用帧

#### Scenario: 槽位耗尽
- **WHEN** 生产快于消费
- **THEN** 丢弃旧帧/不覆盖未释放槽，队列有界，不无限增长

### Requirement: 帧新鲜度与安全帧

系统 SHALL 在帧迟到、停止、静音、接管、worker 失联或旧 epoch 时丢弃旧帧并显示安全帧；SHALL NOT 继续用旧帧配新音频。

#### Scenario: 帧迟到
- **WHEN** 连续帧迟到超过 200ms
- **THEN** 输出安全帧并记录迟到；恢复后从最新帧继续

#### Scenario: 停止后旧帧
- **WHEN** 用户停止后仍有旧 epoch 帧到达
- **THEN** 系统拒收并保持安全帧，不显示旧嘴型

### Requirement: 有界队列与背压

系统 SHALL 对音频、帧、动作、事件队列设置容量、过期与丢弃策略；SHALL 记录丢弃原因与计数，不无界缓冲。

#### Scenario: 队列满
- **WHEN** 某队列达到上限
- **THEN** 按优先级/新鲜度丢弃，并记录；停止/接管不被队列阻塞

### Requirement: 输出画布与帧率

系统 SHALL 默认输出 1280×720 @ 25fps；MVP 可变更为 960×540；SHALL 记录实际 FPS、掉帧与 AV 偏差；不得用单模型理论 FPS 代替整链 FPS。

#### Scenario: 目标帧率
- **WHEN** 进入直播
- **THEN** 系统按配置帧率输出，记录实际值与掉帧率

#### Scenario: 帧率不足
- **WHEN** 实际 FPS 低于配置或掉帧超阈值
- **THEN** 显示降级/告警，记录原因，不静默降低质量
