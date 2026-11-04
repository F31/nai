## ADDED Requirements

### Requirement: Revocable bounded callback audio
系统 SHALL 只从完整 Preplay 复检及有序消费创建有界音频回调对象；回调 SHALL 独立检查 P0、代次与单调期限，不依赖控制线程轮询，且不得生成设备完成 ACK。

#### Scenario: Control thread stalls or owner disappears
- **WHEN** 控制线程停止续租、门控对象析构、换代或 P0 撤权
- **THEN** 旧回调输出静音，且不会因重签租约恢复

#### Scenario: Invalid audio or oversized callback
- **WHEN** PCM 非有限、越界、超预算或目标缓冲超过 10ms
- **THEN** 拒绝准入或清零输出，不将复制样本记为实际播放完成
