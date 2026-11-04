## ADDED Requirements

### Requirement: Isolated approved local media delivery
系统 SHALL 将有界原始媒体与Protobuf授权分离，输出端复检session/epoch/批准哈希/事实/租约；失联600ms内音频静音，不生成硬件完成ACK。

#### Scenario: Root stops renewing
- **WHEN** 根进程退出、EOF或停止续租
- **THEN** keeper独立撤权，不恢复旧音频块，退出或保持静音

#### Scenario: Tampered media or proof
- **WHEN** 文件长度/哈希、候选或审批绑定不符
- **THEN** 拒绝播放且报告明确失败
