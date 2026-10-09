# Proposal

## Why

D02 是 D03 存储、D04 内核与 D05 IPC 的共同前置条件。V3.5 第 4、20–30 章要求跨语言消息只有一份 Schema 源，并在进入消费者开发前建立生成和兼容性验证。

## What Changes

- 建立核心/领域 Protobuf、版本化能力目录及配置/插件/模型 JSON Schema。
- 建立控制 API 的 OpenAPI 契约，接口定义与运行实现状态分开。
- 从源生成 Rust、TS、C++ 消息，TS 的 64 位数采用字符串。
- 实现 `cargo xtask schema [--check]` 和真实 contracts suite，拒绝生成漂移。
- 验证跨语言编码、大整数、未知字段、错误/状态以及错误配置拒绝。
- 更新 CI、契约操作文档、ADR 与任务证据，保持其他业务任务未开始。

## Capabilities

### New Capabilities

- `versioned-contracts`：唯一契约源、确定性多语言生成、结构校验与可测的协议兼容性。

### Modified Capabilities

无；工程工具新增可用命令，未实现命令仍按既有失败语义处理。

## Impact

影响 `schemas/`、contracts、TS 包、C++ 生成目录、xtask、契约测试、CI 与文档。
新增 Protobuf/JSON Schema/OpenAPI 开发工具与编码运行库，无新增产品常驻服务、云主机或 API 调用。
定义 DTO 和结构检查不等于已实现 IPC ACL、审批授权、播放门控或 HTTP 服务。
