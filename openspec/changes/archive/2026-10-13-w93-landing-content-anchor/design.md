# w93 设计：`landing` 的内容锚点

## 一、问题形状

| | 台账在声明什么 | 判据实际验了什么 | 缝在哪 |
| --- | --- | --- | --- |
| `decided_in_repo` | 决定已作出**且已落地** | `landing` 的每个元素是**仓库已跟踪路径** | 「落在哪里」被验了，「**落在那里的东西还在不在**」没人验 |

探针实测（改真实数据 → 跑 `node --test tests/registry.test.mjs` → sha256 还原）：

- **探针①**：`PD-22.landing` → `README.md`（真实存在、`git ls-files` 确认已跟踪）⇒ **50 pass / 0 fail**。
- **探针②**：`landing` 不动，删掉规格里 `PD-24` 的落地句 ⇒ **50 pass / 0 fail**。

两个方向合起来说明：`landing` 的判据**只在看守路径的存在性**。指错文件（只要它存在且被跟踪）不响；
落地内容被撤销也不响。⇒ 「已落地」是一条**判据放过的空声明**。

顺带核实了另两个"看起来已结清"的态（结论：**当前无可订正的数据**，故本轮不动它们）：

- `answered_by_baseline` 4 条：§46.2 表格明写 sampling「**默认禁用**」+ 启用四项条件；§41.4 明写
  「MUST 作为观测项登记，但**本轮不设告警阈值**」；§46.5 明写「**不新增** `mcp.tool@1`／`a2a.*`」；
  §51.3 的能力包清单逐行独立（MCP = H11／H12、A2A = H13／H14）。⇒ **都真的答出了**。
- `needs_precondition` 4 条：H05／H06／H07／H08／H13／H14／H16 **全部** `pending` ＋ `capability_pack`
  ＋「未开始」；PD-30 挂的 `W61`（MCP 适配）其 H11／H12 也 `pending`。⇒ **前置全部未达成**，无迁移可做。

## 二、决定 1：锚点是**平行数组**，不并进 `landing`

| 候选 | 代价 |
| --- | --- |
| **平行数组**：新增 `landing_anchor`，与 `landing` 等长、按位置对应 | 「一一对应」要靠**等长断言**守住；位置错位由「锚点在对应文件里找不到」兜住 |
| **对象数组**：`landing: [{path, anchor}, …]` | `landing` 的元素从字符串变成对象 ⇒ `REPO_PATH_FIELDS` 的解析器要重写、**4 条既有数据全部改形状**；既有的「元素是字符串」前提被打破 |

⇒ 选**平行数组**：`landing` 的形状不变（字符串清单），新增字段是**纯增量**；等长断言 + 逐项内容断言
把「对应」钉住。这也守住「**不合并维度**」——路径与内容是两样东西，写成两层而非塞进一个元素。

## 三、决定 2：锚点是**第五类**，不归入既有的四类

既有四类：`CITATION_FIELDS`（`§NNN`）、`ID_LIST_FIELDS`（登记 id）、`REPO_PATH_FIELDS`（仓库路径）、
`STRUCTURAL_FIELDS`（`id`／`disposition`）。

锚点是「**文件内容证据**」：它不是可解析的引用（不解析成章节／id／路径），也不是结构字段。
⇒ 新增 **`ANCHOR_FIELDS = ['landing_anchor']`**，与四类**两两不相交**。

这也让「**新增一个类别时闭世界断言会点名它**」这条既有纪律再被真跑一次：五类的并集 MUST 恰等于
`LEDGER_FIELDS`，漏登记会在**两个方向**各报一次。`REFERENCE_FIELDS`（"可解析引用"的扫描域）
**不**纳入锚点——它不需要被解析，只需要被**读到并比对**。

## 四、决定 3：`landing` 与 `landing_anchor` MUST 同生同灭（耦合断言）

若锚点**可选**，那么"不给锚点"就是绕过本轮判据的那条缝——洞原样还在。⇒ 新增
`COUPLED_REQUIRED_FIELDS`：`landing` 与 `landing_anchor` 必须**同时存在**且**等长**。

为什么不动 `REQUIRED_FIELD`（一态一个必填字段）的形状：把它扩成"字段列表"会牵动
`REQUIRED_FIELD.get` 的每个使用点；耦合是**两个字段之间的关系**，与「这一态的必填字段是谁」
是两个不同的问题 ⇒ 单独一张表、单独的判据（与 W73 的耦合断言同形）。

## 五、决定 4：文件内容由 `readTrackedText` 注入，读不到 MUST 具名报错

判据要读文件内容 ⇒ `deps` 增一项 `readTrackedText`（同步，`(path) => string | undefined`）。
按既有纪律「**入参显式注入，缺任一项即报错**」，它进入 `required` 清单，缺了 MUST 报
`inputs are required`，MUST NOT 静默跳过。

**读不到文件时 MUST 具名报错**（`could not read <path> — the anchor check must not silently pass`），
MUST NOT 当"通过"：这是「**没看到**」与「**出错了**」的类型分离（铁律二），也是
「不可解析 ⇒ 失败并分类，不静默降级」。

## 六、四条的真实锚点（逐条实测存在）

| 条目 | 落地点 | 锚点 |
| --- | --- | --- |
| PD-09 | `crates/nai-security/Cargo.toml` | `[target.'cfg(windows)'.dependencies]`（**该段**才是"只在 Windows 引入 keyring"这一决定的承载处） |
| PD-09 | `crates/nai-security/src/lib.rs` | `NativeVault` |
| PD-09 | `schemas/platform-matrix/platforms.json` | `secret-protection` |
| PD-22 | `openspec/specs/agent-harness/spec.md` | `R1 首版入站只响应本机与本地信任配置中已登记的受信对端` |
| PD-24 | `openspec/specs/agent-harness/spec.md` | `R1 首版委派链深度上限 MUST 为常量` |
| PD-31 | `crates/nai-kernel/src/evaluate/memory.rs` | `viewer_memory_enabled: false` |

**PD-22 与 PD-24 的落地点是同一个文件、锚点不同** ⇒ 这正说明「落地点相同 ≠ 决定相同」：
没有锚点，两条 `landing` 完全无法区分。

## 七、非空洞注入清单（F1–F8，全部真跑 + sha256 还原）

| # | 注入 | 期望 |
| --- | --- | --- |
| F1 | 把 PD-24 的锚点改成该规格里不存在的串 | **红**（具名：条目 + 字段 + 值） |
| F2 | 删掉 PD-31 的 `landing_anchor` 里一项（长度 ≠ `landing`） | **红** |
| F3 | 删掉 PD-22 的整个 `landing_anchor`（有 `landing` 无锚点） | **红** |
| F4 | 某个锚点给空串 | **红** |
| F5 | 把 `landing_anchor` 挂到**别的处置**的条目上且锚点不存在 | **红**（触发条件按字段存在，不按处置） |
| F6 | 反向：合法锚点挂到别的处置上 | **不红**（本轮拒绝的是"锚点不在"，不是"放错位置"） |
| F7 | `LEDGER_FIELDS` 漏登记 `landing_anchor` | **红**（闭世界） |
| F8 | `shapeErrors()` 的类别并集漏掉 `ANCHOR_FIELDS` | **红**（并集 ≠ `LEDGER_FIELDS`） |

每次注入后 **sha256 逐字还原**（python 写入必须 `newline=''`，否则 LF→CRLF 会造成"内容相同但字节不同"
的**假差异**——本轮核实阶段已实测过一次）。

## 八、明确不做

- **不**改任何 `status`、**不**新增能力、**不**碰 Rust：本轮只让"已落地"这句声明真的被判据守住。
- **不**把锚点做成"全仓唯一"或"该文件特有"——无法判定，且会把判据变脆（假阳性方向）。
- **不**订正 `answered_by_baseline`／`needs_precondition`（见 §一：实测无可订正的数据）。
- **不**声称"已落地已被证明"：锚点验的是"所指文件里有这段内容"，**不**验"这段内容语义上兑现了那个
  决定"，也**不**验"这段内容只应该出现在那里"。剩余的部分仍由人读（ADR 0054 决策 2）。
