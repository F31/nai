# Versioned Contracts

## Purpose

为 nai 会话、能力和插件提供跨语言一致且可验证的版本化数据边界，以唯一 Schema 源生成消费者类型，并让后续内核、媒体、平台与管理实现能够识别无效输入和不兼容协议。

## ADDED Requirements

### Requirement: Single contract source
跨语言消息 SHALL 由 Protobuf 定义并生成 Rust/TS/C++；配置和清单 SHALL 由 JSON Schema 定义，HTTP SHALL 由 OpenAPI 定义；能力目录必须引用存在的正式消息类型，包括 V3.5 的基础与子能力。

#### Scenario: Capability references missing message
- **WHEN** 能力目录出现重复 ID、缺失类型或无正式请求/响应 Schema 的条目
- **THEN** 契约检查失败，不报告目录完整

### Requirement: Deterministic generation
生成工具 SHALL 锁定版本，从源重新生成全部产物；`schema --check` 必须在临时目录生成并拒绝新增、删除或内容漂移，不覆盖消费者文件。

#### Scenario: Generated code drift
- **WHEN** 已生成文件被修改、遗漏或增加陈旧产物
- **THEN** 检查返回非零并指明差异；正常重新生成可恢复一致性

### Requirement: Safe protocol representation
消息 SHALL 使用字符串 ID，TS 的 64 位整数保持字符串；信封必须包含协议、关联 ID、期限及载荷，会话操作必须包含 session/epoch；缺少必需字段、无期限、未知消息类型或不兼容主版本必须拒绝。

#### Scenario: Large epoch and future field
- **WHEN** 三种语言交换超出 JS 安全整数的 epoch，或带同主版本新增未知字段的消息
- **THEN** 已知字段无精度损失且可解码；未知字段不授予权限，未知枚举不当作 granted/pass

#### Scenario: Invalid control message
- **WHEN** 信封过大、主版本不兼容、无剩余期限或会话作用域字段缺失
- **THEN** 返回明确结构错误；编码成功不代表通过业务授权

### Requirement: Validated configuration and API description
配置/插件/模型清单 SHALL 拒绝未知字段、明文密钥位置、非法资源/期限及缺失必需组件；OpenAPI 必须描述鉴权、命令幂等/状态版本、会话 epoch 与实际 ACK，且保留端点尚未实现的状态。

#### Scenario: Invalid manifest or config
- **WHEN** 配置包含未声明字段、危险数值或凭据明文，模型包缺少图/I/O/许可/兼容矩阵
- **THEN** JSON Schema 校验失败，不报告插件/模型 Ready

### Requirement: Verified compatibility boundary
契约 suite SHALL 实际编译 Rust/TS，并在可用环境编译 C++ 生成代码，执行三语言互通、未知字段和协议范围测试；缺 C++ 环境必须失败或明确 Blocked，不能将生成成功视为编译或互通通过。

#### Scenario: Run contract suite
- **WHEN** 运行具备匹配工具/运行库的 contracts suite
- **THEN** 实际生成检查、配置/API 校验和跨语言数据交换通过后才报告成功
