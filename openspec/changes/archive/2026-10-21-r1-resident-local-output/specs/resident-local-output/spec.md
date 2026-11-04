## ADDED Requirements
### Requirement: 会话常驻输出窗口
系统 SHALL 在同一引擎会话内复用受监督的 Windows 输出进程及窗口，每句独立校验审批、哈希、事实版本、epoch和短租约，句间静音待机。
#### Scenario: 两句独立批准内容
- **WHEN** 操作员依次播放两个不同已批准候选
- **THEN** 同一 keeper PID 和窗口完成两次提交，句间不重复调用Provider或重播
#### Scenario: 控制失联
- **WHEN** 私有管道关闭或播放租约不再续期
- **THEN** 音频撤权，未完成内容不能报告硬件完成，输出不会自动恢复
#### Scenario: 有界输入
- **WHEN** 单句超过20秒、帧超过64MiB或队列溢出
- **THEN** 拒绝并保持静音，不截断成成功
#### Scenario: 播放中人工撤权
- **WHEN** 播放中静音或人工接管使当前epoch失效
- **THEN** keeper撤销当前句并回静音待机，确认超时则被回收，旧续租不能自行重播
