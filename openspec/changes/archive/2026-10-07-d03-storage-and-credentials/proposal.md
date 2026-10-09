# Proposal

## Why

D02 已提供生成契约，但状态、权威事实和凭据仍没有持久化实现。
本轮落实 D03 及 V3.5 第 14、24 章，为后续会话、知识与 Provider 提供可验证数据基础。

## What Changes

- 创建 nai-storage/nai-security，建立完整表目录、编号迁移和 checksum 漂移检查。
- 单写 owner、外键、WAL/FULL 持久化、有限等待及取消/期限；不暴露原始数据库连接给插件。
- 实现带权限/CAS 的配置与事实存储，以及审批/事实失效的持久状态和撤销通知记录。
- 使用 SQLite 在线备份，加上有界的已登记不可变文件及校验清单；恢复至全新目录，保留历史并撤销旧运行权限。
- Windows 使用系统 Credential Manager，配置/数据库只存 credential_ref；非 Windows 显式 Unavailable。
- 软件凭据 broker 检查当前授权，不宣称已实现原生插件 OS 沙箱。
- 新增 storage suite、真实迁移/恢复/取消测试，保持 Windows 凭据实测独立。

## Capabilities

### New Capabilities

- `local-persistence`：单机持久化、迁移、权限/版本一致性及安全备份恢复。
- `credential-boundary`：系统密钥存储、引用与授权访问、明确不可用及脱敏。

### Modified Capabilities

无。新增备份 JSON Schema；生成契约与工程边界不改变已有语义。

## Impact

新增 migrations/sqlite、两个共享 crate、备份 Schema、测试、xtask/CI 与文档。
增加嵌入式 SQLite、Schema 校验与系统凭据依赖，无新增常驻数据库、云服务或模型请求。
本轮持久撤销不代表 D06/D09 已执行媒体静音或缓存内存撤销。
