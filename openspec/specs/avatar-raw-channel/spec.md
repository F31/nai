# avatar-raw-channel Specification

## Purpose
定义本机独立原始音频通道及与签名控制之间的两阶段消费关系，要求私有密钥互证、单块背压和有限长度读取，并确保半包与输入结束后的执行取消仍可到达；本机组件验证不替代生产worker装配或GPU播放验收。

## Requirements

### Requirement: 原始通道独立且取消可达
本机原始PCM组件 SHALL 与签名控制分离，挑战认证私有子进程密钥、限制元数据与PCM大小；分段读取不得阻塞控制取消。

#### Scenario: 原始包未完整到达
- **WHEN** 已签名Chunk占槽但只收到部分原始PCM
- **THEN** 同一任务Cancel仍能通过控制线程处理，有限时间内停止原始读且不发消费成功回执。

#### Scenario: EOS之后仍在执行
- **WHEN** 输入EOS已验证但计算或输出尚未结束
- **THEN** Cancel仍能撤销任务，不能将输入结束当作拒绝取消的理由。
