# D02 版本化契约与跨语言生成

## 任务状态

- 任务：D02；变更：`d02-versioned-contracts`；关联 ADR 0003。
- 实现完成：Schema 源、能力目录、生成工具、结构检查与 contracts suite。
- Linux/WSL 实测：Rust/TS/C++ 编译、结构/兼容样本、生成漂移和前端检查通过。
- Windows：CI 已扩展，尚未实际执行；Windows C++、设备、SDK、安装与 GPU 未认证。
- D03–D19 未开始；IPC 身份/ACL、业务审批/事实校验、播放租约及 HTTP 服务未实现。
- 未提交或推送代码；本轮未调用真实 LLM/TTS/Embedding API。

## 契约与生成目录

- `schemas/core`：上下文、状态、错误、信封、握手、取消、能力与绑定。
- `schemas/domain`：平台、知识/权威事实、决策/守门/审批/技能/节目、Provider、媒体、控制/存储/观测。
- 共 10 个 proto 文件；27 个能力条目；95 个 descriptor 消息名（含 map-entry 内部类型）。
- 7 个 JSON Schema：目录、runtime、provider、plugin、model、avatar、declarative UI。
- 控制 OpenAPI：路径、鉴权、权限、幂等/状态版本、session/epoch、审批 hash、实际 ACK 与 job 状态。
- 文本产物：`crates/nai-contracts/src/generated`、`packages/contracts-ts/src/generated`、`native/contracts/generated`。
- 仅 target 保存 descriptor、fixture 二进制、C++ 可执行文件和中间构建；未下载真实模型/素材。

## 环境与工具锁定

Linux/WSL，Rust 1.96.0、Node 22.23.1、pnpm 8.15.0、OpenSpec 1.14.1。
protoc/libprotobuf 3.21.12、GNU C++ 13.3.0、CMake 3.28.3。
prost/prost-build/prost-types 0.14.4、ts-proto 2.13.0、@bufbuild/protobuf 2.16.0。
AJV 8.20.0、Swagger Parser 13.1.0、json-schema-to-typescript 16.0.0、openapi-typescript 7.13.0。
所有 Cargo/pnpm 依赖由工具重新解析锁定；非手工锁文件。

## 实际命令与结果

| 命令 | 最终结果 |
|---|---|
| `apt-get install -y protobuf-compiler=3.21.12-8.2ubuntu0.3 libprotobuf-dev=3.21.12-8.2ubuntu0.3` | 成功；本机补齐生成/C++ 环境 |
| `protoc --version`、`pkg-config --modversion protobuf` | 3.21.12 |
| `cargo generate-lockfile`、`pnpm install` | 成功更新真实锁文件 |
| `pnpm install --frozen-lockfile` | 成功 |
| `cargo xtask schema` | Rust/TS/C++、JSON Schema TS、HTTP TS 生成成功 |
| contracts suite 内 `schema --check` | 临时重建，无文件集合/内容漂移 |
| `cargo xtask test --suite contracts` | Rust 测试、TS 编译、7 项 Node 校验、C++ 编译及两条互通路径全部通过 |
| `cargo xtask check --profile portable` | 依赖边界、fmt/clippy 和 16 项 Rust 测试通过 |
| `pnpm typecheck`、`pnpm build` | contracts 与 desktop 均通过 |
| `OPENSPEC_TELEMETRY=0 pnpm spec:validate` | D02 变更及已有主规格 strict 校验通过 |
| `pnpm exec openspec archive d02-versioned-contracts --yes`（遥测关闭） | 10/10 任务完成；5 条需求合并主规格并成功归档 |
| 归档后 `pnpm spec:validate` 与 `openspec validate --archived --no-interactive`（遥测关闭） | 3 个主规格及 D01/D02 两项归档均通过 |

归档位置：[D02 proposal/specs/design/tasks](../../openspec/changes/archive/2026-10-07-d02-versioned-contracts/)。
新增主规格：`openspec/specs/versioned-contracts/spec.md`。

### 测试覆盖与边界

Rust 共 **16 项**：D01 依赖/CLI 9 项保留，新增 5 项协议检查及 2 项生成/目录测试。
生成负向测试在真实临时文件夹构造修改/缺失/陈旧文件，验证检查不修改消费者，
重新同步清理陈旧产物；目录重复/不存在消息必须失败。

Node 共 **7 项**：有效结构、未知字段、deadline/权限条件、凭据明文与 URL、
模型图依赖/I/O/路径、形象许可字段/UI 受控组件、目录类型/基础能力覆盖、
OpenAPI 鉴权/epoch/revision/hash/ACK/job 与错误码对应关系。

两条互通路径：

- Rust → TS → C++ → Rust。
- Rust → C++ → TS → Rust（C++ 直接接收包含未来字段的样本）。

每条验证 7 类消息：Envelope、PerformanceCue、AudioBlockMetadata、PermissionSnapshot、
ContractError、CapabilityResult、AuthoritativeFact。
包含超 JS 安全范围的 epoch/seq、uint64 最大值、中文、未知字段 99、未知枚举 77，
以及 exact money oneof（129900 分 / CNY / scale=2）。
这些结果证明本轮样本和结构的互通，不证明业务授权、实际设备/模型或任意未来版本兼容。

## 发现、失败与修复

1. npm/Cargo 下载出现暂时网络重置；工具重试后完成，保留真实锁文件。
2. proto 顶层枚举值在 C++ 作用域冲突：PermissionState 使用完整前缀，保留规范 ErrorCode 名称。
3. OpenAPI 初稿 flow 字符串未引用逗号、YAML merge 键未被解析器支持；改为明确字段与普通 anchors，通过完整校验。
4. TS barrel 的通配导出重复导出生成器内部 helper；从 descriptor 明确导出消息/枚举名称。
5. JS descriptor 重新编码丢失 `oneof_index=0`，被金额互通样本捕获；改为 prost-types 保留 optional 存在性，TS 正确生成联合类型。
6. 生成 suite 初次超过 120s 工具超时，未记 Passed；增加执行时限后完成真实全链路检查。
7. 一轮 WSL/mounted filesystem CMake 出现时钟偏移警告；后续完整重建无该警告且全部互通通过。

## 迁移、成本与后续任务

无已有消费者或持久数据迁移。config 预算使用最小单位字符串；Schema 通过不授予 Ready/许可。
新增的是开发工具、生成文本与 Protobuf 编码运行库；没有常驻云依赖、模型推理服务或 API 消耗。
本机 apt 补齐约 15.6MB 开发依赖，不属于正式安装器已认证资源预算。

下一项 **D03：存储与密钥**，先新建 OpenSpec 变更；建立 SQLite 表/迁移、一致备份、
凭据引用和权限/事实版本测试。Windows 安全凭据后端实测与 portable 逻辑分别记录。
