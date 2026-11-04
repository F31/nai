# 个人偏好迁移日志契约

新增迁移日志 JSON Schema 并由项目生成 TypeScript 类型。契约固定 foundation 源版本 2、目标版本 3，记录迁移 ID、备份 ID、清单 SHA-256 和有限阶段（prepared、backup_verified、committed）；不允许额外字段或由日志表达回滚授权。日志字段本身不是备份真实存在、哈希正确或迁移已完成的证明，后续必须由可信存储适配器实际核验。

验证：使用项目生成器 `cargo xtask schema` 及 `schema --check` 通过；个人策略与迁移日志定向契约测试各 1 项通过；活动 OpenSpec 严格校验通过。沙箱内 protoc 遇到 Windows DLL 错误，审批后使用项目 E 盘工具成功生成及检查。构建和缓存位于 `target-windows/avatar-contract-generation`。

尚未实现：版本 3 数据库 schema、持久日志读写、阶段原子转换、实际备份身份校验、故障恢复、偏好存储和 IPC/UI；当前程序仍使用版本 2，不会自动迁移。版本 3 仅为本次拟议迁移预留，不意味着已发布或可以直接修改现有数据库；原有预算、审批与验收注册表保持不变。
