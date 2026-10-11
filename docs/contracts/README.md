# nai v1 契约维护

## 唯一来源与产物

| 来源 | 语义 | 产物 |
|---|---|---|
| `schemas/core/*.proto` | 信封、上下文、错误、版本、状态与能力视图 | Rust/TS/C++ 消息 |
| `schemas/domain/*.proto` | 平台、知识/事实、候选/审批、Provider、媒体、控制 | Rust/TS/C++ 消息 |
| `schemas/catalog/capabilities.json` | 能力目录：27 条 **Frozen**（正式请求/响应、流/会话作用域）与 6 条 **Draft**（§37.4 开放契约：无消息、不生成消费者代码） | Rust 类型目录、目录类型检查 |
| `schemas/config/*.schema.json` | 运行及 Provider 配置 | 校验与 TS 配置类型 |
| `schemas/plugin/*.schema.json` | 插件、模型、形象、声明式 UI | 校验与 TS 清单类型 |
| `schemas/openapi/control.yaml` | HTTP 路径、请求/响应、鉴权与 ACK | TS HTTP 类型、Rust 嵌入校验 schema |

生成目录：`crates/nai-contracts/src/generated`、`packages/contracts-ts/src/generated`、
`native/contracts/generated`。这些目录归生成器所有，不能手改。
Rust 入口为 `nai_contracts::v1`，TS 入口为 `@nai/contracts`。

## 工具准备

除项目固定 Rust/Node/pnpm 外，需要 **protoc 3.21.12**。
Ubuntu 24.04 可以安装 `protobuf-compiler`（核对 `protoc --version`）；
Windows 使用上游 v21.12 的 `protoc-21.12-win64.zip`，将 bin 加入 PATH。
也可通过 `PROTOC` 指向匹配编译器；工具会验证版本。
不需要在产品运行机安装 protoc 或 Node 生成工具。

完整 C++ suite 另需 CMake、C++17 编译器及 **libprotobuf 3.21.12**。
Ubuntu 安装 `libprotobuf-dev`；Windows 需配置同版库的 CMake 查找路径。
缺运行库时 suite 失败，不输出 Passed；仅生成 C++ 代码不是 C++ 编译通过。

## 命令

```bash
pnpm install --frozen-lockfile
cargo xtask schema
cargo xtask schema --check
cargo xtask test --suite contracts
```

`schema --check` 在 target 临时重建，再与消费者文件逐项比较；不会修复漂移。
`schema` 会重新同步整个 generated 目录并删除陈旧生成文件。
生成器不得借 git diff 判断一致性；未提交状态下也必须能正确检查。

完整 suite 实际运行：

1. 无漂移重生成与能力目录类型解析。
2. Rust 协议检查/协商测试。
3. TS 编译及 JSON Schema/OpenAPI 正负样本。
4. 编译全部 C++ 生成消息并链接同版 libprotobuf。
5. Rust → TS → C++ → Rust 和 Rust → C++ → TS → Rust：
   信封、候选、音频元数据、权限、错误、流结束、权威事实、绑定报告、预算账本与平台 epoch/连接代次共 10 类样本。

样本包含 `9007199254740993`、uint64 最大值、中文、未来字段、未知枚举 77，
以及 129900 分 / CNY / scale=2 的 exact money oneof、u64 最大预算金额与取消后未知实际账单；全部只用于测试。

## 语义边界

- TS 64 位数为字符串，禁止业务中无检查地转成 Number。
- string ID 不按平台大整数转换为 JS number。
- 未指定/未知 PermissionState、GuardVerdict、SessionMode 不表示授权或通过。
- Envelope 结构检查不证明 payload 的业务内容、对端身份、审批、事实或租约有效。
- 工作站收到的期限转成本地单调截止；D06/D07 提供租约与调度门控，跨节点时钟偏差验收仍待接线。
- BudgetCharge/UsageLedgerEntry 只声明精确金额/用量/计费状态，不自报全局或 Provider 授权额度；执行见 [调度/预算指南](../scheduler.md)。
- 媒体只在控制消息传元数据/buffer 引用，PCM/帧及所有权通道由 D05/D11 实现。
- 模型清单的结构/引用闭包通过不证明 G0–G3 或资源认证。
- 控制 OpenAPI 本轮只有显式 loopback 工程状态读取/mute/takeover 子集已实现，其他操作保持未实现；见 [控制指南](../control.md)。resource_ref 是受控后端引用，不是任意 URL/SQL/路径。
- SessionCommand 新增必需 device_id/seq，旧 HTTP 请求缺字段拒绝；`control_http::SCHEMAS_JSON` 由源扁平化 allOf 后闭合字段，不手写或手改 schema。实际 control ACK 不等于硬件 ACK。
- 旧名 adapter 尚未实现，不能自动当新契约使用。
- 能力目录的**每条**都声明阶段（`stage`：`frozen` / `draft`）。**只有 `frozen` 生成消费者代码**
  （`MESSAGE_TYPES`、`SESSION_MESSAGE_TYPES` 与协商用的 `CAPABILITY_IDS`）；`draft` 只表示「已登记、未冻结」，
  **MUST NOT** 声明 `request`/`response`/`stream`/`session_scoped`，也 **MUST NOT** 出现在协商面里
  （它另列于 `DRAFT_CAPABILITY_IDS`）。验收注册表引用一个 `draft` 契约只说明**名字在册**，
  **不**说明该验收通过、也不改变任何 `status`。
- 目录的「完整模型」（类别、必需性、首次出现的切片、缺失时的降级行为）**尚未落地**，
  `stage` 是本轮唯一新增的字段；不得据此宣称目录已完整。
- 门控分工：静态 schema 校验（`tests/contracts/schema.test.mjs`）**不需要** protoc，已并入
  `cargo xtask check --profile portable|full`；需要编译描述符的校验
  （`tests/contracts/descriptor.test.mjs`）留在 `cargo xtask test --suite contracts`。

## 变更步骤

先建立 OpenSpec 变更和场景，再改源、生成、运行 suite、更新消费者。
字段编号不复用，删除字段须 reserved；不兼容语义升主版本并记录 ADR。
最后更新计划/报告并归档；真实认证环境另行记录。
