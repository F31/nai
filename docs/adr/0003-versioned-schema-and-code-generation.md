# ADR 0003：版本化契约与跨语言生成

- 日期：2026-10-07
- 状态：Accepted
- 对应：D02；V3.5 第 4、20–30 章

## 决策

1. 消息源为 `schemas/core` 与 `schemas/domain` 的 Protobuf `nai.v1`。
   配置/能力目录/插件/模型/形象/声明式 UI 用 JSON Schema 2020-12，HTTP 用 OpenAPI 3.0.3。
   不手写三份领域 DTO。核心 ErrorCode 与 OpenAPI 错误枚举保持一致并测试。
2. protoc 3.21.12（上游发行标签 v21.12）、prost/prost-build/prost-types 0.14.4、
   ts-proto 2.13.0、TS 编码运行库 @bufbuild/protobuf 2.16.0。
   JSON TS 生成器 json-schema-to-typescript 16.0.0；HTTP TS 生成器 openapi-typescript 7.13.0。
   AJV 8.20.0、Swagger Parser 13.1.0 验证结构；真实解析锁入 lockfiles。
3. `schema` 先在 target 暂存完整生成，再同步专属 generated 目录；
   `schema --check` 仅比较消费者目录的文件集合和内容，发现修改/缺失/陈旧文件即失败。
   descriptor 和互通二进制只在 target 中，不进入正式运行包。
4. TS `forceLong=string`；显式 proto3 optional 保留 epoch/时间的存在性。
   `oneof=unions` 生成互斥事实值，货币使用有符号最小单位/币种/scale，decimal 使用字符串。
   runtime 配置的预算由原文浮点示意 `per_session_limit` 映射为 `per_session_limit_minor` 字符串，
   避免跨语言精度歧义；默认配置示例属于测试，不代表经营建议。
5. ts-proto 插件由 Node 直接运行；插件请求用 prost-types 构造，保留 proto2 descriptor
   的 optional 存在性。不得用丢失零值存在性的 JS descriptor 编码器重建请求，
   否则 `oneof_index = 0` 会丢失并错误生成 TS 类型。
6. 基础能力名按 V3.5 原名保留。可选子能力明确登记为
   `embedding.generate@1`、`rerank.rank@1`、`moderation.check@1`、
   `classification.classify@1`、`skill.list@1`、`gateway.invoke@1`。
   旧名只列 `adapters_required`，不提供静默重命名或声称 adapter 已实现。
7. 信封检查负责编码上限（当前为完整编码信封 64KiB）、必需字段、协议主版本、
   已知类型、剩余期限以及目录声明的会话作用域；协商选择双方协议范围交集。
   不检查真实身份、当前 epoch、事实、审批密码学或租约执行。
8. OpenAPI 定义认证、权限、revision/epoch、最终对象 hash、实际 ACK、202 job 语义。
   资源管理初始使用服务端受控 resource/config/upload 引用；资源视图不返回密钥/媒体。
   端点均标为未实现；细化资源数据和服务实现由 D14/D15 的独立变更完成。
9. 配置验证由 AJV 结构检查加小范围关系检查组成（租约间隔、缓冲水位、图依赖闭包、
   URL 凭据、数值范围）。校验通过不授予 Ready、素材许可或平台权限。

## 兼容策略

v1 首次建立，无已发布消费者。新可选字段及枚举扩展需未知值安全回归；删除字段必须
保留编号/名称（reserved），不兼容行为升主版本。TS/prost 不承诺保留未知字段，
只承诺解码已知字段且未知值不授予权限；转发原始未知字段如有需求须另建实现。
未来发布版本的兼容范围仍须逐组合认证，不由本轮样本推出任意未来兼容。

## 验证与成本

Linux/WSL 真实执行 Rust/TS/C++ 编译、两条三语言路径、结构/无漂移检查。
C++ 要求精确 3.21.12 运行库；缺环境不跳过并伪报 suite 通过。
Windows 基础生成/类型检查纳入 CI，未实际运行；Windows C++/SDK/设备认证未执行。
本轮只新增开发工具、生成文本及 Protobuf 编码运行库，无云服务或付费模型请求。
