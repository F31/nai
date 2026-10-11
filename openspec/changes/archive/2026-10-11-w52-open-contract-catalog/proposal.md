## Why

设计 §37.4（第 1635 行）承诺 `data.query@1`、`data.subscribe@1`、`device.sense@1`、`device.control@1`
**先以 Draft 加入注册表**，`asr.transcribe@1` 为**预留**，`market.catalog@1` 为**客户端契约**；
第 149 行要求能力目录的每一条都带**状态（Draft / Frozen / Deprecated）**；第 63 行规定
「只有被某个切片验收用例覆盖的契约才能 Frozen，其余保持 Draft，**不生成消费者代码**」。
第 151 行把这六个连同 `skill.invoke` 一并列为「Draft，自 R1 起逐步冻结」。

对照仓库现状，这条承诺**没有落点**：

- `schemas/catalog/capabilities.json`（§4.1 的**唯一来源**）只有 **27** 条，**没有阶段字段**，
  且这六个契约**一条都不在**。
- 而 `tests/registry.yaml` 已经**引用了其中三处**：`market.catalog@1` ×6（M13–M18）、
  `data.query@1`（M04）、`device.control@1`。即「开放兼容」的验收条目挂在**目录里不存在的能力**上。
- 更关键的是**门控抓不到**：`scripts/contracts/registry.mjs` 只校验 id 形状、唯一性、`release_line`
  与 `status` 枚举，`contract` 字段**完全没有校验**。于是 `plugin-hosts-and-ipc`（架构标签）与
  `market.catalog@1`（悬空引用）在门控眼里没有区别，「有名无册」可以长期静默存在。

同时本轮补上一条**门控盲区**：`tests/contracts/schema.test.mjs` 由 CI 步骤 19 运行，但**不在**
`check --profile portable|full` 的任何一步里。W51 改了 `schemas/config/backup.schema.json` 的
`db_schema_version` 常量却没同步该测试的样本，于是 **CI 在 ubuntu 与 windows 双平台红在步骤 19，
而本机 portable 全绿**（详见 design §5 与报告勘误段）。门控必须能在本机复现 CI 的结论。

## What Changes

- **能力目录引入阶段字段**：`capabilities[].stage: "draft" | "frozen"`；现有 27 条标 `frozen`，
  六个开放契约以 `draft` 补入（`minItems` 27→33）。
- **生成器区分两阶段**（`xtask/src/schema.rs`）：只有 `frozen` 的条目参与 `MESSAGE_TYPES`、
  `SESSION_MESSAGE_TYPES` 与 `CAPABILITY_IDS`（即**只有 frozen 生成消费者代码**）；`draft` 另出
  `DRAFT_CAPABILITY_IDS`，且 **draft 不得声明 `request`/`response`/`stream`/`session_scoped`**。
- **注册表门控新增契约可解析校验**（`scripts/contracts/registry.mjs`）：每条验收的 `contract`
  必须命中目录条目，或命中**具名非能力契约白名单**（当前仅 `plugin-hosts-and-ipc`，附理由）。
- **闭掉门控盲区**：把 `tests/contracts/schema.test.mjs` 中**不依赖 protoc 产物**的静态 schema 校验
  纳入 `xtask` 的工程门控（`engineering_checks`），依赖 `target/contracts/descriptor.bin` 的目录类型
  解析拆到 `tests/contracts/catalog.test.mjs`，两者仍都由 contracts suite 与 CI 运行。
- 文档：`docs/contracts/README.md` 更新能力计数与阶段语义；`docs/development-plan.md`、`AGENTS.md`、
  新报告；W51 报告**追加勘误**（按「历史记录不改写、只追加勘误」的既有规则）。
- **不做**：不改任何既有验收 `status`；不为 draft 发明消息类型或降级行为；不实现任何开放契约的执行体；
  不引入新依赖、不新增 xtask 子命令。

## Capabilities

### Modified Capabilities

- `versioned-contracts`：把「能力目录必须引用存在的正式消息类型」推广为「目录**每条**须声明阶段，
  **只有 Frozen 条目**引用正式消息并生成消费者类型，Draft 条目 MUST NOT 声明请求/响应」；
  并新增一条需求：**注册表里每个契约引用必须可解析**。

## Impact

- 修改：`schemas/catalog/capabilities.json`、`schemas/catalog/catalog.schema.json`、
  `xtask/src/schema.rs`、`xtask/src/lib.rs`、`scripts/contracts/registry.mjs`、
  `scripts/contracts/validation.mjs`、`tests/registry.test.mjs`、`tests/contracts/schema.test.mjs`、
  `.github/workflows/foundation.yml`、`docs/contracts/README.md`、`docs/development-plan.md`、`AGENTS.md`。
- 新增：`tests/contracts/catalog.test.mjs`、`crates/nai-contracts/src/generated/catalog.rs`（生成物更新）、
  `packages/contracts-ts/src/generated/config/catalog.ts`（生成物更新）、
  `docs/reports/2026-10-11-w52-open-contract-catalog.md`。
- **零新增依赖**：仍 18 包；变化都在既有 JSON/Proto/Rust/Node 面内。
- **已知后果（如实登记，不掩盖）**：`crates/nai-storage/src/snapshot.rs` 用 `hash(capabilities.json)`
  作为备份清单的 `contract_catalog_sha256`，因此**本次改动会改变该哈希**：早于本轮的备份在本轮构建上
  报告 `VersionIncompatible`。这与「draft 不进消费者代码」并不矛盾（哈希覆盖整个文件），属**既有设计的
  过严之处**；是否把哈希收窄为「仅 Frozen 条目」是独立决定，本轮**不改**，登记为观察项。
- **验证**：`check-deps`、`check --registry`、`cargo xtask schema`（本机受同步子进程阻断所限，见 design §5）、
  `check --profile portable`、`pnpm test:ui`、`node --test tests/contracts/schema.test.mjs`、
  `openspec validate --all --strict --archived`；CI 观测两次推送的 `foundation` 两平台结论。
