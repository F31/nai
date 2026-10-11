# 设计：开放契约册与门控

## 1. 为什么目录需要「阶段」这个字段

设计第 63 行给契约定义了三态：**Draft → Frozen(v1) → Deprecated**，并给出冻结的判据是
「**只有被某个切片验收用例覆盖的契约才能 Frozen**」，其余保持 Draft 且**不生成消费者代码**。
第 149 行进一步要求能力目录的每一项都带这个状态。也就是说，阶段不是文档上的形容词，而是
**决定生成面**的开关：Draft 契约可以出现在登记表、路标与验收里，但**不得**进入协商清单、
消息类型表与生成的消费者代码。

仓库现状没有这个开关，于是产生一个**不可判定的状态**：六个开放契约既然无处登记，就无法被门控
判定为「已声明但未冻结」还是「根本不存在」。前者是正常路标，后者是悬空引用——本变更把两者分开。

## 2. `frozen` 的判据（本仓口径）与一处如实登记的偏差

本变更给 `frozen` 的判据是**可机械核验**的：目录条目声明了 `request`/`response` 且二者都能在
编译出的 proto 描述符里解析到，并因此进入 `MESSAGE_TYPES`/`SESSION_MESSAGE_TYPES`/`CAPABILITY_IDS`。

**偏差（如实登记）**：设计第 825 行把 `memory.access`、`skill.invoke`、`show.plan`、
`storage.persist`、`runtime.observe` 列为「保持 Draft，在对应切片使用时再冻结」，而本仓这五条
**已有正式消息绑定且已生成消费者代码**，其中 `skill.invoke@1` §37.4 明写「在 R1 冻结」。
按第 63 行的更强判据（须被切片验收覆盖），这五条里仍有未达标者。本变更**不做改判**：
若把它们降为 draft，会立刻把它们移出 `CAPABILITY_IDS`，等于让内核与 SDK 的协商面收缩——
那是**功能回退**，不是文档订正。故本轮按「有正式绑定即 frozen」的**仓库既有事实**标注，
并把「按第 63 行更强判据重新裁定这五条」登记为**待裁决项**，不混进本轮。

## 3. Draft 的具体含义（生成面）

- `MESSAGE_TYPES` / `SESSION_MESSAGE_TYPES` / `CAPABILITY_IDS` **只由 frozen 条目产生**——这就是
  「不生成消费者代码」的可执行含义。
- 新增 `DRAFT_CAPABILITY_IDS`，只列出已声明但未冻结的 id。它的用途是**可判定**：门控与测试据此
  断言「六个开放契约确实在册」，且断言它们**不在** `CAPABILITY_IDS` 里。
- **draft 不得声明** `request`/`response`/`stream`/`session_scoped`：draft 若半声明一条消息，
  就同时是「已冻结」和「未冻结」，阶段字段就失去意义。这一条在 **JSON Schema 与 Rust 生成器两侧**都查，
  不靠单侧。
- 本轮**不**为 draft 发明消息类型、类别、降级行为。设计第 149 行要求的「类别/必需性/首次切片/
  缺失时的降级行为」是更完整的目录模型，**尚未做**——本变更只落地决定生成面的那一个字段，
  不假装目录已经完整。

## 4. 注册表门控：契约必须可解析

`registry.mjs` 新增两项：读取 `schemas/catalog/capabilities.json` 的 id 全集（frozen + draft），
以及校验每条验收的 `contract` 取值。

判据是**二选一**：命中目录 id（任意阶段都算「已声明」），或命中**具名非能力契约白名单**。
白名单当前只有一项：

| 取值 | 理由 |
|---|---|
| `plugin-hosts-and-ipc` | A06 描述的是**架构性质**（多宿主与信任边界），不是线上能力；目录登记的是可调用契约，此值不属于其中。 |

白名单刻意保持极小并在代码里就地附理由：每加一项都必须解释「为什么它不是能力」。
`contract` 的缺失（字段不存在）不是错误——部分验收本就不绑定契约。

## 5. 门控盲区：本机 portable 绿而 CI 步骤 19 红

**事实**：`.github/workflows/foundation.yml` 的步骤 19 运行 `node --test tests/contracts/schema.test.mjs`，
而 `check --profile portable|full` 的 `engineering_checks` 依次只跑 `check-deps`、`check --registry`、
`fmt`、`clippy`、`cargo test --workspace`、`cargo test -p nai-xtask --lib`——**不含**该测试。
W51 把 `backup.schema.json` 的 `db_schema_version` 常量由 5 改成 6，却漏改该测试里的样本，结果是
**CI 两平台都红在步骤 19，而本机 portable 全绿**（见报告勘误段）。这不是判据问题，是**门控覆盖问题**。

**处理**：把该测试文件按**是否需要 protoc 产物**拆开。

- `tests/contracts/schema.test.mjs`：纯 JSON Schema / OpenAPI 静态校验（Ajv + swagger-parser），
  **不需要** `target/contracts/descriptor.bin` ⇒ 并入 `engineering_checks`。
- `tests/contracts/catalog.test.mjs`：能力目录类型要对**编译出的描述符**解析 ⇒ 留在
  `cargo xtask test --suite contracts`，CI 步骤 19 改为同时运行两个文件。

可行性依据：`engineering_checks` **本就**不是纯 Rust——`registry::check` 经 `node --test
tests/registry.test.mjs` 运行，需要 node 与仓库的 npm 依赖（`yaml`）。并入一个同为 node+npm 的静态
schema 校验，**不引入新类别的工具链前置**（protoc 仍只在生成与 contracts suite 需要）。
`check --profile portable` 的「不含前端」承诺（缺 node_modules 时不红）不受影响的理由不变：
两者都需要已安装的依赖，这是**既有事实**，本变更不改变它，也不把它扩大。

## 6. 已知后果与风险

1. **备份契约哈希漂移（已知、接受、登记）**：`snapshot.rs` 的 `contract_catalog_sha256` 是
   `hash(capabilities.json)` 整体哈希。加入 draft 条目会改变它 ⇒ 早于本轮的备份在本轮构建上被判
   `VersionIncompatible`。当前无生产数据，可接受；把哈希收窄为「仅 Frozen 条目」是独立决定（需在 Rust
   侧对 JSON 做确定性重建），本轮**不做**，作为观察项登记。
2. **生成物重建**：`generated/catalog.rs` 与 `packages/contracts-ts/src/generated/config/catalog.ts`
   必须随源重建，否则 `schema --check` 会报漂移。本机同步子进程被阻断（`spawnSync`/`execSync` 一律
   `EBUSY`，异步 `spawn` 正常），故生成按既有做法**临时**把 `generate.mjs` 的 `spawnSync` 换成等价
   异步 shim、跑完**还原脚本**——不改仓库源码来迁就主机限制。
3. **不升级任何 `status`**：M04/M13–M18 的契约引用由「悬空」变为「已声明（Draft）」，但
   `admission-registry` 的诚实回填规则禁止据此把任何目标升为 `tested`。本轮不碰 `status`。

## 7. 验收与不升级

- 门控：`check --registry` 必须能**拒绝**一个指向不存在契约的条目（用测试的负样本证明，而不是靠
  本次恰好通过）。
- 生成：`MESSAGE_TYPES`/`SESSION_MESSAGE_TYPES`/`CAPABILITY_IDS` 中**不得**出现 draft id；
  `DRAFT_CAPABILITY_IDS` 必须**恰好**是六个开放契约。
- 目录：`catalog.schema.json` 必须拒绝「draft 声明了 request/response/stream/session_scoped」的条目。
- 不宣称：目录仍不完整（缺类别/必需性/首次切片/降级行为）；六个开放契约**没有实现、没有消息、
  没有降级行为**；本轮**未**产生任何可用于真实直播的能力。
