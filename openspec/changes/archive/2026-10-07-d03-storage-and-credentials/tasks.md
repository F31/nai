# Tasks

## 1. 规范及数据源

- [x] 1.1 完成 D03 四类工件并通过 strict 校验。
- [x] 1.2 建立备份清单 JSON Schema 并重新生成/校验契约。
- [x] 1.3 建立完整表目录及编号迁移；测试事务回滚、checksum/版本漂移和外键。

## 2. 存储与权限

- [x] 2.1 实现单 owner、OS 用户路径、有限等待/取消；以唯一写者与期限测试验证。
- [x] 2.2 实现配置/事实/审批权限、CAS 和失效 outbox；验证跨域拒绝及事务一致性。
- [x] 2.3 实现有界不可变文件、在线快照和新目录恢复；验证 WAL、损坏/路径/覆盖拒绝与旧授权失效。

## 3. 密钥与工程集成

- [x] 3.1 实现 Windows NativeVault、非 Windows Unavailable、broker 与 zeroized/redacted Secret；验证权限/撤销/取消与脱敏，Windows 实测单列。
- [x] 3.2 实现 storage suite、CI 与 ADR/操作指南；通过匹配测试与可用的 Windows 编译检查。

## 4. 验证与交付

- [x] 4.1 运行 portable/storage、contracts、前端及 OpenSpec 检查，记录真实结果和外部环境。
- [x] 4.2 更新开发计划、AGENTS/README 和报告，保持 D04 为下一项。

## Workflow follow-up

- 验证后标准归档并重新检查主规格和归档任务。
