# w93：`decided_in_repo` 的落地点 MUST 带内容证据 —— `landing` 补上锚点

## Why

`decided_in_repo` 这个态断言的是「**决定已作出，且已落地**」。它的必填字段是 `landing`（落在仓库的
哪里），而判据只钉住一件事：**每个元素是本仓库已跟踪的路径**。

两个探针实测（改真实台账／真实规格 → 跑 `node --test tests/registry.test.mjs`）：

| 探针 | 改动 | 结果 |
| --- | --- | --- |
| ① | 把 `PD-22.landing` 从 `openspec/specs/agent-harness/spec.md` 换成 `README.md`（**真实存在、且已被 git 跟踪**） | **50 / 50 全绿** |
| ② | `landing` 不动，把 `PD-24` 在规格里的落地句「**R1 首版委派链深度上限 MUST 为常量、MUST NOT 提供配置面**」删掉 | **50 / 50 全绿** |

⇒ 「已落地」在当前判据下是一条**被放过的空声明**：指着任意一个已跟踪文件都算落地，落地点里的
内容被撤销了也仍然算落地。这是「**已声明 ≠ 已执行**（无执行体即假契约）」的形状——台账说"已落地"，
判据只验"路径存在"，两者之间那一段没有任何人看守。

**这个风险是本仓库自己刚造大的**：W92 把 `decided_in_repo` 从 **2 条扩到 4 条**（PD-22／PD-24 落到
活规格），这 4 条现在全靠"路径存在"这一条判据支撑它们的"已落地"。

同时核实了另外两个"看起来已结清"的态，**结论是它们当前没有可订正的数据**：
`answered_by_baseline` 的 4 条 `basis` 都真的答出了问题（§46.2 明写 sampling 默认禁用、§41.4 明写
不设告警阈值、§46.5 明写不新增契约名、§51.3 的能力包清单逐行独立）；`needs_precondition` 的 4 条
前置**全部未达成**（H05／H06／H07／H08／H13／H14／H16 均 `pending`，PD-30 挂的 W61 其 H11／H12
也 `pending`）。⇒ 本轮只补 `decided_in_repo` 这一格。

## What Changes

- **新增字段 `landing_anchor`**：与 `landing` **等长、逐项对应**的字符串清单；第 *i* 个锚点 MUST
  真实出现在第 *i* 个落地点文件的**内容**里。
- **新增第五类检查 `ANCHOR_FIELDS`**：锚点是「文件内容证据」，与既有的四类（`§NNN` 引用／登记 id／
  仓库路径／结构字段）**两两不相交**；闭世界的「类别并集恰等于 `LEDGER_FIELDS`」随之为五类。
- **新增耦合断言**：`landing` 与 `landing_anchor` **同生同灭**且**等长**——有落地点就 MUST 有锚点，
  反之亦然；否则"没配锚点"会成为绕过本轮判据的那条缝。
- **新增依赖 `readTrackedText`**：判据要读文件内容，入参显式注入；读不到 MUST **具名报错**，
  MUST NOT 静默降级（「没看到」与「出错了」在类型上分开）。
- **4 条 `decided_in_repo` 各配真实锚点**（PD-09 三个路径、PD-22、PD-24、PD-31）。

## Impact

- `scripts/contracts/pending-decisions.mjs`：`ANCHOR_FIELDS`、`COUPLED_REQUIRED_FIELDS`、
  `readTrackedText` 依赖、`shapeErrors()` 的类别划分（四类 → 五类）、闭世界 `LEDGER_FIELDS` 加
  `landing_anchor`。`REFERENCE_FIELDS` **不**纳入锚点——它不是"可解析的引用"，是"内容证据"。
- `tests/pending-decisions.yaml`：4 条加 `landing_anchor`（**数据改动**，每条实测存在）。
- `tests/registry.test.mjs`：注入 `readTrackedText`、在「缺输入必报错」清单里加它、新增锚点相关用例。
- **不改任何 `status`、不新增能力**：本轮只让"已落地"这句声明**真的被判据守住**。
- **锚点守什么、不守什么（如实披露）**：锚点把门槛从「路径存在」提高到「**所指文件里真有那段内容**」，
  把"随手改一个 landing"变成"必须同时伪造一个内容证据"。但它**守不住**「把 `landing` 指到另一个
  恰好也含该锚点的文件」——判据验的是"你指的那个文件里有这个串"，不是"这个串只应该出现在那里"。
  ⇒ 这是**签名 + 观测**的分工，不是全知验证；本轮在 ADR 与报告里写明，MUST NOT 被读作"已落地已被证明"。
