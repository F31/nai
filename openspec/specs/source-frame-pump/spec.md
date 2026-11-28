# source-frame-pump Specification

## Purpose
定义唯一语音生产线程与Rust帧控制泵之间的单槽背压，确保上游网络等待不阻塞像素复检和释放；统一应用时钟记录源首包、供应商验证及首帧，失败取消并回收生产和GPU，不将部分输出或单次时长波动视为实时通过。

## Requirements

### Requirement: 网络等待不阻塞帧消费
应用 SHALL 将唯一TTS生产与owner帧处理分离，以单槽背压交付PCM，在源网络暂停期间仍复检和释放帧。

#### Scenario: 供应商暂停下一包
- **WHEN** 当前GPU块已生成而供应商尚未交付下一PCM包
- **THEN** Rust仍处理有效Frame和Release，不等网络包到达才清空帧通道。

### Requirement: 同起点计时和失败关闭
应用 SHALL 用同一起点记录首PCM、供应商验证和首验证帧；失败取消生产及GPU，不把部分帧或消息断连当作EOS。

#### Scenario: 消息槽满时取消
- **WHEN** 上游生产者因单槽满而等待且父任务取消
- **THEN** 有限时间退出等待并回收，不增加无界缓存或自动重试。
