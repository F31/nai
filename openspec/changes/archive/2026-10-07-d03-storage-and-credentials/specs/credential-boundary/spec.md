# Credential Boundary

## Purpose

把 nai 的配置引用和 API 密钥本体分离，让可信装配层可以提供有授权的凭据访问，同时防止不可用环境用明文文件或测试替身假装系统密钥存储已就绪。

## ADDED Requirements

### Requirement: Native secret storage and references
Windows 实现 SHALL 使用系统凭据存储，配置/数据库/备份只保存 credential_ref；无后端时显式 Unavailable，不自动落明文或使用 mock。返回密钥的 Debug/错误信息必须脱敏，持有缓冲释放时清理。

#### Scenario: Missing native backend
- **WHEN** 非 Windows 环境调用生产凭据后端
- **THEN** 返回 Unavailable，不产生密钥文件或伪报保存成功

### Requirement: Authorized bounded credential access
凭据 broker SHALL 每次检查当前 principal 的获准引用及 deadline/cancellation，撤销立即影响后续读取；principal 由可信装配/后续身份边界提供，软件 broker 不冒充原生宿主 OS 沙箱。

#### Scenario: Unauthorized or revoked reference
- **WHEN** principal 未获准某引用或授权被撤销
- **THEN** 返回权限拒绝且不调用后端读取秘密

### Requirement: Honest native verification
Windows 凭据测试 SHALL 使用真实系统后端且清理仅测试创建的条目；Linux 替身结果不能标为 Windows 系统存储通过。

#### Scenario: Native verification unavailable
- **WHEN** 当前执行环境没有 Windows Credential Manager
- **THEN** 报告保留 Windows 实测 Blocked，继续独立的权限/脱敏与存储测试
