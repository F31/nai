# W137：O05 的成本来源——把「升级成本显示」接上控制面

日期：2026-10-17　基线：`docs/直播数字人 + 智能体系统技术方案 V3.11.md`（§1330 O05、§30.1、§900）
ADR：[0112](adr/0112-upgrade-plan-cost-source.md)　变更：`openspec/changes/archive/2026-10-17-w137-upgrade-plan-cost/`

## 一、勘定（先量，再动手）

### 1.1 四环现测

| 环 | 现测结果 |
| --- | --- |
| ① 模型 | **已有**：`CostEstimate`（`crates/nai-upgrade-core/src/cost.rs`），四分量 `Component::Known(u64)` / `Unknown(&'static str)`，`total()` 给 `Complete` / `Partial{floor,unknown}`，`tests/gates.rs` 覆盖 |
| ② 执行体 | **没有**：控制面 `handle` 的权限匹配与分派里对 `/upgrades/plan` **0 命中**；`schemas/openapi/control.yaml` 声明了 `/upgrades/plan`、`/upgrades/execute`、`/upgrades/{id}/rollback`，三条都未标 `implemented-loopback` |
| ③ 调用方 | **一半**：`apps/desktop/src/upgrade.ts` 有 `parseCostEstimate`/`costRows`/`totalPresentation`（19 项），`apps/desktop/src/control.ts` 的 `ControlClient` **没有**任何升级方法 |
| ④ 真实产生者 | 包清单（含 `cost`）来自厂商发布的升级包；仓库内只有结构校验（`UpgradePackage::parse` + `schemas/config/upgrade-package.schema.json`） |

缺口的形状因此是**②与③之间少一条线**，而①的答案本来就在仓库里 ⇒ 这一轮的性质是**接线**，
不是造模型。

### 1.2 缝上量到的三件事（都不是推测）

1. **契约给 plan 复用了作业信封**。`/upgrades/plan` 的 `responses` 是 `*jobResponses`（`202` + `JobAccepted`），
   而 plan **不下载、不暂存、不迁移** ⇒ 没有任何可轮询的进行中状态；铸一个 `job_id` 却永远查不到东西，
   是**伪造可观测面**。
2. **体上限是命令口径**。所有 POST 共用 `4096` 字节，而包清单允许到 `MANIFEST_LIMIT = 65_536`
   ⇒ 一个合法清单会变成静默的 413。
3. **413 当时不可观测**。拒绝后 `break` 并回响应、关闭连接，对端因为还有未读数据在途收到
   **RST**（实测 `reqwest … ConnectionAborted / Os 10053`）⇒ 「拒绝了」成了只有服务端知道的状态。
   这条是本轮写用例时**实测撞到**的，不是读代码推出来的。

### 1.3 两处口径不一致，处置选拒绝

| | 清单 schema（`schemas/config/upgrade-package.schema.json`） | 呈现契约（`CostView` / `parseCostEstimate`） |
| --- | --- | --- |
| `currency` | `minLength 1, maxLength 8` | `^[A-Z]{3}$` |
| 金额 | `^(0|[1-9][0-9]*)$` 的十进制字符串，无上界 | 非负**安全整数**（`Number.isSafeInteger`） |

两条口径不一致时可选三条路：合并（改一处）、放宽（改呈现判据）、拒绝（挡在控制面）。
本轮选**拒绝**：既有呈现判据有 19 项覆盖，放宽是拿判据换覆盖率；而合并会动到清单 schema（外部契约）。
于是控制面在 plan 路径上挡两类包：币种不能呈现、金额超过 `2^53-1`。

## 二、交付

### 2.1 契约（`schemas/openapi/control.yaml`）

- `/upgrades/plan` 由「202 + `jobResponses`」改为 **200 + `UpgradePlanView`**，打
  `x-implementation-status: implemented-loopback`，权限仍 `runtime.upgrade`；
- 新增 `UpgradePlanRequest`（只认 `manifest` 一个键）、`UpgradePlanView`（`package_id`、
  `package_version`、`cost`）、`CostView`、`CostComponents`、`CostLine`（`oneOf[{known},{unknown}]`，
  两层 `additionalProperties: false`）；
- **清单结构不在 OpenAPI 里重述**：`manifest` 只是 `type: object`，真相只有
  `schemas/config/upgrade-package.schema.json` 一份（控制面读同一份）；
- `scripts/contracts/openapi-ops.mjs` 的 `OPENAPI_DOCUMENTS.implemented` 增 `post /upgrades/plan`。

生成物由 `cargo xtask schema` **本机再生**（`protoc 3.21.12` 已就位）：
`crates/nai-contracts/src/generated/control_http.rs`（`SCHEMAS_JSON` 增五个 `$defs`）与
`packages/contracts-ts/src/generated/http.ts`。

### 2.2 控制面（`crates/nai-control-api`）

- 依赖 `nai-upgrade-core`（`shared → shared`，`xtask check-deps` 允许）；后者只多导出 `MANIFEST_LIMIT`，
  避免控制面另立一个同值数字。
- `upgrade_plan`：`plan_schema` 校验（`$defs/UpgradePlanRequest`）→ `UpgradePackage::parse` →
  `CostEstimate::from_package` → 四分量搬运。**成本只算一处**。
- 三道表示性闸门，各自具名、各自不产出成本面：
  `package currency is not a three-letter code` / `package cost exceeds the representable amount range` /
  `package manifest rejected: <stable reason token>`。
- `COMMAND_BODY_LIMIT = 4096` 与 `PLAN_BODY_LIMIT = MANIFEST_LIMIT + 1024` 分开；超限后**有界排空**
  （`body_limit × 4`）再回 413，使拒绝可观测。

### 2.3 桌面（`apps/desktop`）

- `parseUpgradePlan`：只认三个键（多一个也拒）、身份非空、`cost` **一律**交给 `parseCostEstimate`
  ——信封不重判金额，判别性用例保证「成本解析只有一处」不是口号；
- `ControlClient.planUpgrade(manifest)`：`POST /upgrades/plan`，请求体**只有** `manifest`；
- `COST_DECLARED_BASIS_NOTE`：「金额来自包清单声明，未验签；执行前由升级事务校验签名」——
  本路径不验签是**设计**（请求里没有包体），但界面必须说出来，否则就把「别人说多少钱」显示成了
  「要花多少钱」。

### 2.4 顺带订正的前提：6 处「本机验不了契约」的过期表述

W136 结尾装好的 `protoc 3.21.12` 让 `cargo xtask schema --check` 第一次能在本机跑绿，于是
**6 处**已发布记录里的「本机缺/无 protoc」「`schema --check` 跑不了 ⇒ CI-only」全部过期：
`docs/development-plan.md` 4 处（W129 / W130 两处 + 更早那句以 `EBUSY` 为由判 CI-only 的）、
`docs/knowledge.md` 2 处。按 §1 的分载体规则，这些都是**已发布轮次记录** ⇒ **追加勘误段，不改原文**
（dev-plan 写在 W137 小节「勘误」，knowledge 追加文末「勘误」节）。

## 三、判据

### 3.1 控制面（`crates/nai-control-api/src/tests.rs`，+9）

| 用例 | 钉的是什么 |
| --- | --- |
| `the_plan_route_needs_the_upgrade_permission_and_says_nothing_else` | 缺 `runtime.upgrade` ⇒ 403，且**响应里没有成本面**（「被拒」与「算出来是 0」分开） |
| `the_plan_view_satisfies_the_contract_it_declares` | 响应交给**它自己声明的** `UpgradePlanView`（多一个 `total` 也会红） |
| `a_component_the_manifest_leaves_out_is_a_reason_and_never_zero` | 未声明 ⇒ `unknown` 带非空原因，无 `known` |
| `an_explicit_zero_is_reported_as_zero` | 显式 `"0"` ⇒ `known: 0`（与上一条互为反向） |
| `a_manifest_that_fails_its_schema_is_named_and_yields_no_estimate` | 清单不合规 ⇒ 400 具名，且无成本面 |
| `a_request_carrying_anything_beside_the_manifest_is_refused` | 多一个顶层键 ⇒ 400；连 `manifest` 都不给 ⇒ 400 |
| `a_currency_that_cannot_be_presented_is_refused_rather_than_widened` | `cny` ⇒ 400 具名 |
| `an_amount_past_the_representable_range_is_refused` | `2^53` ⇒ 400 具名 |
| `the_plan_body_limit_follows_the_manifest_limit_not_the_command_limit` | 约 30KB 的合法清单 ⇒ 200；刚超 plan 上限 ⇒ 413 且具名 |

### 3.2 桌面（`tests/ui/upgrade-plan.test.mjs`，新增 10）

信封严格性 3 条（只认三键 / 多为难 / 缺键与空身份）、成本单一来源 1 条（**同时含一致性样本与判别性样本**）、
呈现口径 3 条（未知是原因、含未知是下界、全已知是合计）、来源边界内容锚点 1 条、客户端请求形状 1 条、
坏响应拒绝 1 条。

### 3.3 注入（9 组，逐组真红）

注入基线：`cargo fmt --all` 后 `--check` 通过，四个文件 `sha256sum` 记录；还原后 `sha256sum -c` 全部 `OK`。

| # | 注入 | 变红的用例 |
| --- | --- | --- |
| I1 | `Unknown(reason)` → `{"known": 0}` | `a_component_the_manifest_leaves_out…` |
| I2 | 币种闸门改成 `is_empty()` | `a_currency_that_cannot_be_presented…` |
| I3 | 去掉金额上界（`Known(amount)` 无条件） | `an_amount_past_the_representable_range…` |
| I4 | 权限映射降为 `runtime.read` | `the_plan_route_needs_the_upgrade_permission…`（200 ≠ 403） |
| I5 | 体上限退回命令口径 | `the_plan_body_limit_follows…`（合法清单得 413） |
| I6 | 清单解析失败 ⇒ 200 + 空成本 | `a_manifest_that_fails_its_schema…`（200 ≠ 400） |
| I7 | 跳过请求体 schema 校验 | `a_request_carrying_anything_beside…`（200 ≠ 400） |
| I8 | 信封不判多余键 | 「多一个看起来方便的字段就被拒绝」+「不合规的计划响应被拒绝」 |
| I9 | `cost: value.cost as CostEstimate`（透传） | 「成本解析只有一处」（判别性样本） |

I8/I9 这两组说明一件事：信封层的两条判据各自**独立**可红——去掉多余键检查，判别性样本仍绿；
改成本透传，只有判别性样本红，一致性样本（`deepEqual` 两条路径同结果）照样绿。

## 四、门控

命令与环境：

```
PATH=<rustup 1.96.0-x86_64-pc-windows-msvc>/bin:$PATH   CARGO_HOME=…  RUSTUP_HOME=…
PROTOC=C:/Users/32791/.local/toolcache/protoc-21.12/bin/protoc.exe
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo xtask check --profile full
```

`GATE_EXIT=0`，日志 4061 行，`grep -c LNK1104` = **0**，`^(error|failures:|thread … panicked)` 零命中。

| 面 | 计数 | 结论 |
| --- | --- | --- |
| Rust | 101 个测试目标 / **1112 passed / 0 failed** | 绿 |
| Node（13 个非前端套件） | 57 / 30 / 22 / 14 / 18 / 24 / 34 / 22 / 16 / 16 / 15 / 12 / 11 = **291** | fail 0 |
| 前端 `test:ui` | **62**（含本轮新增 10 条） | fail 0 |
| 前端 `apps/desktop → test` | 5 | fail 0 |
| Node 侧合计 | 291 + 62 + 5 = **358 pass / 0 fail** | 绿 |

### 4.1 本轮遇到的 `LNK1104`（宿主瞬态，非代码缺陷）

本轮共 7 次红在 `LNK1104`：1 次 `cargo test -p nai-control-api --lib`、5 次手工预链接、1 次整门控；
**目标至少 6 个互不相同**（`nai_control_api-*`、`render-*`、`candidate_hash-*`、`protocol-*`、
`control_http-c2bba549833113e3`、`nai_contracts-da5d610c86001883`），且报错指向的 exe **在盘上不存在**。

已排除：磁盘空间（49G 可用）、目录可写（同路径 `touch` 成功）、残留链接进程、杀毒进程、代码缺陷
（单目标单跑全绿）。已**证伪**的做法：逐目标预热——单包构建的 exe 哈希（`protocol-d7e2453d66b90620`）
与工作区构建（`protocol-4df7628a6b5d4fb7`）不同，预热产物不被工作区构建复用。

最终处置：`CARGO_INCREMENTAL=0` + `CARGO_BUILD_JOBS=1` 下**有界重试**（20 次 × 8s）预链接，
第 3 次 `EXIT=0`（`PREBUILD_GREEN`），随后门控一次即绿。**未改任何代码、未放宽任何判据**——
与 MEMORY §2 记录的 W136 处置同族，本轮再次复现同一形状。

## 五、具名登记（发现但不做）

1. **无真实渲染的面板**。`apps/desktop/src/main.tsx` 没有升级界面，`tests/ui` 也没有 DOM 环境
   ⇒ 「显示」这个动词还没有主语。`O05` 保持 `pending`，与 O02 同惯例。
2. **`execute` / `rollback` 仍是未实现的作业**。契约里那两条保持 `202 + job_id`，控制面 0 命中。
   它们真的要做多步事务，作业语义是对的；本轮没动。
3. **plan 不含 preflight 事实**（磁盘、平台、依赖闭包）。`HostFacts` / `Probe` 的真实来源不在仓库内，
   塞进来就是编造。
4. **总额不在服务端算**。`Total` 在 `nai-upgrade-core` 有实现，呈现口径的合计/下界在桌面端
   （`totalPresentation`）。本轮**不**新增第三个实现，也不把两者合并（一个是模型、一个是措辞）。
5. **不验签**。plan 路径只有清单、没有包体；这条边界写进契约描述、ADR、`COST_DECLARED_BASIS_NOTE`
   与计划的呈现说明，MUST NOT 在别处被读成「已校验」。
6. **排空是有限度的**。超过 `body_limit × 4` 的体仍会以 RST 结束 —— 这是**已知边界**，不是判据的边界：
   用例刻意只超出上限 4KB 之内，正是为了不与这条边界混淆。

## 六、CI 观测

_（回填）_
