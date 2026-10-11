# W52：开放契约册与门控（O02 之外的「有名无册」收口）

日期：2026-10-11　　变更：`w52-open-contract-catalog`（已归档）

## 一、本轮两件事

**第一件是修 W51 的回归**：W51 的 CI **不是绿的**，是 `failure`（见 §二）。我没有把它当成「顺手修一下」，
因为它暴露的是一条**门控覆盖缺陷**，而不是一次笔误——同一类缺陷还会再发生。两件事同属「契约册与门控」，
所以放在同一次提交里，但报告分开写清楚。

**第二件是把「开放兼容」的地基补上**：设计 §37.4/§149/§63 要求能力目录的每条都带阶段（Draft/Frozen），
六个开放契约先以 Draft 入册。仓库此前**没有阶段字段**、六个契约**一条不在册**，而 `tests/registry.yaml`
已经**引用了其中三处**（`market.catalog@1` ×6、`data.query@1`、`device.control@1`），门控却**抓不到**。

## 二、W51 的 CI 实为红：更正与根因

我上一轮报告的「`foundation` 两 job 全绿」**来自一次被 HTTP 代理缓存的过期响应**。以缓存破坏参数重查后：

| 提交 | run | 结论 | 失败步骤 |
|---|---|---|---|
| `347d74e`（W51） | `38133956560` | **failure**（ubuntu + windows 两个 job） | 步骤 19「Validate schemas and API contract」 |
| `0e1c233`（平台化审视，docs-only） | `38134889196` | **failure**（继承同一断点） | 同上 |

**根因**：W51 把 `schemas/config/backup.schema.json` 的 `db_schema_version` 常量由 5 改成 6，
却没有同步 `tests/contracts/schema.test.mjs:86` 的 backup 样本（仍写 `5`），该用例两侧不再一致。

**为什么本机没发现（这是重点）**：`check --profile portable|full` 的 `engineering_checks` 依次只跑
`check-deps`、`check --registry`、`fmt`、`clippy`、`cargo test --workspace`、`cargo test -p nai-xtask --lib`，
**不含** `tests/contracts/schema.test.mjs`；CI 却用**独立步骤**运行它。于是出现
「**本机门控绿、CI 红**」——门控在最需要它的时候给了假绿灯。W51 报告已追加**勘误段**（原文不改写）。

## 三、能力目录的阶段语义（本轮落地的部分）

- `schemas/catalog/catalog.schema.json`：条目改为 `oneOf` 两支——`frozen`（须有 `request`/`response`/`stream`）
  与 `draft`（**不得**有这四个字段）；`required` 增 `stage`；`minItems` 27→33。
- `schemas/catalog/capabilities.json`：现有 **27 条标 `frozen`**，新增 **6 条 `draft`**——
  `data.query@1`、`data.subscribe@1`、`device.sense@1`、`device.control@1`、`asr.transcribe@1`、`market.catalog@1`。
- 生成器（`xtask/src/schema.rs`）：**只有 frozen 生成消费者代码**——`MESSAGE_TYPES`、
  `SESSION_MESSAGE_TYPES`、协商用的 `CAPABILITY_IDS` 都只取 frozen；draft 另列
  `DRAFT_CAPABILITY_IDS`（这就是「不生成消费者代码」的可执行含义）。draft 若声明消费者字段，
  **JSON Schema 与 Rust 两侧都报错**（不靠单侧）。
- 生成实测：`catalog checked: 33 capabilities (27 frozen, 6 draft); 112 protobuf messages`；
  `generated/catalog.rs` +9 行（新增 `DRAFT_CAPABILITY_IDS`）。

**如实登记两处不完整**：

1. `frozen` 的判据在本轮是**可机械核验**的（有正式消息绑定且能解析、生成消费者代码），
   而设计第 63 行的更强判据是「被切片验收覆盖」。按更强判据，`memory.access`/`show.plan`/
   `storage.persist`/`runtime.observe` 等仍未必达标。**本轮不改判**：把它们降为 draft 会立刻把它们
   移出协商面，那是功能回退而非文档订正。已登记为**待裁决项**。
2. 设计第 149 行要的完整目录模型（类别、必需性、首次出现的切片、降级行为）**尚未落地**；
   `stage` 是本轮**唯一**新增字段，不得据此宣称目录已完整。

## 四、注册表门控：契约必须可解析

`scripts/contracts/registry.mjs` 新增读目录 id 集合与逐条校验：

- 每条验收的 `contract` 必须**命中目录条目**（draft 也算「已声明」）或命中**具名非能力契约白名单**；
- 白名单只有一项 `plugin-hosts-and-ipc`，就地附理由（A06 是架构性质，不是可调用契约）；
- **不传目录本身即为错误**（防止门控被「忘了传参数」绕过）；
- `contract` 缺失不算错（部分验收本就不绑定契约）。

`tests/registry.test.mjs` 补正负样本：悬空的 `market.catalog@9` 必被指名报错；
引用 draft 的 M04 通过校验但 `status` 仍为 `pending`（**draft 不使任何验收升级**）。
`scripts/contracts/validation.mjs` 的语义检查收紧为「adapter 目标必须是 **frozen**」，并复述 draft 禁声明消费者字段。

## 五、门控盲区的闭法

按「是否需要 protoc 产物」把契约测试拆开：

| 文件 | 需要什么 | 谁运行 |
|---|---|---|
| `tests/contracts/schema.test.mjs` | 仅 Ajv / swagger-parser（**无 protoc**） | **`check --profile portable\|full`**、contracts suite、CI 步骤 19 |
| `tests/contracts/descriptor.test.mjs` | `target/contracts/descriptor.bin`（protoc 产物） | contracts suite、CI 步骤 19 |

`xtask` 的 `engineering_checks` 现按顺序跑：`check-deps` → `check --registry` →
**`node --test tests/contracts/schema.test.mjs`** → `fmt` → `clippy` → workspace test → xtask lib test。
可行性依据是 `engineering_checks` **本就**依赖 node 与仓库根依赖（`check --registry` 经 node 运行），
并入一个同类步骤**不引入新类别的工具链前置**；`check --profile portable` 的「不含前端」边界不变
（两侧都不需要前端包自己的依赖）。同时订正了 `lib.rs` 里那句**不准确**的注释：portable 从来不是
「只需 Rust 工具链」，真实前置是 Rust + node + 仓库根依赖。

## 六、验证

| 检查 | 结果 |
|---|---|
| `cargo xtask check --profile full`（本机） | **EXIT=0**；83 目标（66 `Running` + 17 `Doc-tests`）、**726 passed / 0 failed**；前端 5 步全通过（`tests/ui` **37/37**） |
| 新门控步骤是否真的跑 | 是——日志里 `admission registry check passed` 之后紧跟 `TAP version 13 / all config/manifest/catalog schemas…` |
| `node --test tests/registry.test.mjs` | **2/2**（含新增的悬空引用负样本） |
| `node --test tests/contracts/schema.test.mjs` | **11/11**（含新增的 draft-set 与半声明负样本） |
| `node --test tests/contracts/descriptor.test.mjs` | **2/2** |
| `cargo xtask schema`（临时异步 shim） | `33 capabilities (27 frozen, 6 draft)`；生成脚本**已还原**（`git diff` 为空） |
| `tsc -p packages/contracts-ts/tsconfig.json` | **EXIT=0** |
| `openspec validate … --strict` → 归档 → `--archived` | valid → 归档 → 全绿 |

## 七、未做与遗留（如实）

- **六个 draft 契约没有实现**：无消息、无流语义、无降级行为、不生成消费者代码。
  本轮只让它们**在册可判**，**未**产生任何可用于真实直播的能力。
- **备份契约哈希会漂移**：`snapshot.rs` 的 `contract_catalog_sha256` 是 `capabilities.json` 的**整体哈希**，
  加 draft 条目会改变它 ⇒ 早于本轮的备份在本轮构建上报 `VersionIncompatible`。当前无生产数据，
  可接受；把哈希收窄为「仅 Frozen 条目」是**独立决定**（需在 Rust 侧对 JSON 做确定性重建），本轮**不做**，
  登记为观察项。
- **`catalog.ts` 生成物膨胀**：`minItems` + `items` 让 `json-schema-to-typescript` 生成
  **33 元组**（原为 27 元组，是**既有**行为，非本轮引入），文件由 213 行变 561 行。属生成器可读性债，
  登记为观察项（改法是去掉 `minItems` 或换生成策略，均超出本轮）。
- **不改任何 `status`**：M04/M13–M18 的契约引用由「悬空」变为「已声明（Draft）」，按
  `admission-registry` 的诚实回填规则**不得**据此升级。
- **W51 的失败 run 记录永久留在仓库**（GitHub 不支持删除 run 历史）。
