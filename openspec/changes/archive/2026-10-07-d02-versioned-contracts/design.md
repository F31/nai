# Design

## Context

D01 工程已有但没有业务消费者。V3.5 第 4/20 章要求完整能力及子能力的正式消息源，
第 21–30 章规定会话、候选、知识、Provider、媒体、控制和清单字段。
本机 Linux/WSL 可安装 protoc/C++ 库 3.21.12；Windows 正式 IPC/SDK 验收仍未开展。

## Goals / Non-Goals

**Goals:** 为 D03–D05 提供生成契约、目录、配置/API 描述和真实兼容检查。

**Non-Goals:** 实现生命周期/IPC framing/ACL、审批密码学、租约执行、Provider 或 HTTP 服务。

## Decisions

1. Protobuf `nai.v1` 包，core/domain 分文件；Rust 聚合生成，TS/C++ 保持源目录。
   proto3 optional 用于必须区分未提供与零值的 epoch/时间，枚举零值代表 unspecified/unknown。
   控制通道只定义音频/帧元数据及 buffer 引用，不传 PCM/帧；媒体数据面另行实施。
2. 锁定 protoc 3.21.12、prost/prost-build 0.14.4、ts-proto 2.13.0。
   TS 使用 forceLong=string，禁用 JSON codecs；HTTP 类型从 OpenAPI 生成，不手写同义 DTO。
   相对在构建时静默下载 latest，显式校验工具版本，可在 Windows/Linux 重现。
3. `cargo xtask schema` 在 target 下临时目录生成 Rust/TS/C++、OpenAPI TS 和 descriptor，
   成功后同步专属 generated 目录；`--check` 对文件集合/内容做比较，不更改消费者。
   检查陈旧文件；不依赖 git diff，因为当前文件尚未提交。
   descriptor 保留生成注释所需的 source info，仅放在 target，供目录和兼容测试使用，不提交二进制。
4. 能力目录为配置/协议的源，条目明确 ID、请求/响应消息与流语义。
   旧契约列显式 adapter 需求，不假装兼容 adapter 已实现。
5. Rust contracts 提供生成消息重导出与小范围纯结构检查：信封大小、字段、消息类型、
   期限与协议范围协商；配置检查使用 AJV 2020-12，OpenAPI 使用 Swagger Parser。
   运行时仍须执行权限/哈希/事实/epoch/租约等业务复检。
6. contracts suite 执行 Rust 测试、TS 类型/编码、JSON/OpenAPI 校验、C++ 编译与链式互通。
   公共固定测试样本由 Rust fixture writer 输出，TS/C++ 解码再编码并交由 Rust复验；
   unknown 字段通过追加合法 varint 注入；不要求 TS/prost 保留未知字段。
7. CI 两平台做生成漂移及 Rust/TS/配置检查；Linux job 增加 C++ 完整 suite。
   Windows protoc 由已锁定发行包提供；Windows C++ 未配环境则不称为其编译已通过。

## Risks / Trade-offs

- 领域定义可能后续增补 → 同主版本仅兼容添加，删除字段需 reserved，行为不兼容升主版本。
- 生成产物较多 → 只提交文本类型，descriptor/测试二进制留在 target，生成目录不手改。
- 外部工具缺失 → 非零退出并说明安装/匹配版本，禁止跳过 C++ 后输出 suite Passed。
- 配置与 protobuf 不能承担完整运行安全 → 文档和报告区分结构验证与后续任务执行。

## Migration Plan

无持久数据迁移。新增契约版本 1，无旧部署消费者。
CI 从 D01 检查扩展到 D02；未实现其他 suite/certify/package 仍拒绝。

## Verification

`cargo xtask schema --check`、`cargo xtask test --suite contracts`、portable、pnpm typecheck/build、
OpenSpec strict validation。追加负向漂移测试，不改真实生成文件；报告所有工具版本及 Windows 边界。
