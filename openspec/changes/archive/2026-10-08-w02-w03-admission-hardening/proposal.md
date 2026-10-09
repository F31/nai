## Why
接管 W02/W03 后发现日志逐条冲刷与主规格原子语义不一致，且缓冲失败时新遥测可越过旧记录；Session 控制撤权没有清除值守租约，静音/接管后仍可准入自动回复。

## What Changes
- 遥测缓冲与后续新遥测同事务按序提交，失败完整回滚；critical 不先冲刷遥测。
- 静音、接管、停止清除值守与自动模式；撤权计数耗尽时值守门控仍 fail closed。
- 回归验证并更新 R07/S01/S09 证据，保留真实 ACK、runtime 与设备缺口。

## Capabilities
### New Capabilities
无。
### Modified Capabilities
- `decision-log-integrity`: 原子有序冲刷和关键写入优先。
- `duty-lease`: 控制撤权后的自动准入关闭与异常路径 fail closed。

## Impact
修改 nai-storage、nai-kernel、测试与文档；既有 schema 5、哈希格式、API 及备份契约不变，无新增依赖/进程。属于 V3.6 W02/W03 完整性修复，不是完整 R07 或 W05 验收。
