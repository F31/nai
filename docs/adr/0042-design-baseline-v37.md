# ADR 0042：设计基线升为 V3.7（V3.6 降为历史参考）

- 状态：接受
- 日期：2026-10-11
- 相关：[ADR 0015](0015-v36-baseline-and-admission-registry.md)（V3.6 升基线，本 ADR 替代其决策 1）、[ADR 0001](0001-project-name-and-design-baseline.md)、
  `docs/design.md`、`README.md`、`docs/development-plan.md`、`AGENTS.md`、`docs/engineering.md`、
  `openspec/config.yaml`、`openspec/specs/admission-registry|duty-lease|decision-log-integrity`、
  `tests/registry.yaml`、`schemas/tests/registry.schema.json`、`xtask/src/registry.rs`

## 背景

[ADR 0015](0015-v36-baseline-and-admission-registry.md)（2026-10-08）把唯一实现基线从 V3.5 升为 **V3.6**，并裁决了 V3.6 内部的一批编号/口径矛盾。
但仓库里同时躺着 **V3.7** 文档（`docs/直播数字人 + 智能体系统技术方案 V3.7.md`，自首轮工程基线提交
`ae31aa6` 起就在仓库中），且它**早已被当作基线使用**——最近一轮编写的 6 篇 ADR **全部**以 V3.7 引用章节：

- ADR 0034（写实 Gate 判定边界）：基线 V3.7 §8.1/§8.2/§8.4/§26.5/§12.5/§655/§1311
- ADR 0035（手机监控面边界）：基线 V3.7 §3.1/§10.2/§37.3/§663/§636/§86/§742/§30.1
- ADR 0036（设备准入边界）：基线 V3.7 §34.4/§1469/§35.3/INV-23/§8.5/§1261/§30.1/§1329/§1424
- ADR 0037（平台生命周期边界）：基线 V3.7 §3.3/§9.2/§27.1/§347/§349/§119/§1075/§1261/§30.1/§1328
- ADR 0038（真实 Provider 证据边界）：基线 V3.7 §30.1/§16 S07/§1469/§5.9/§12.5/§655/§1329
- ADR 0039（Provider 消息轮次）：基线 V3.7 §30.1、ADR 0038、W19

与此同时，**基线指针从未同步**：`README.md`、`docs/design.md`、`docs/development-plan.md`、`AGENTS.md`、
`docs/engineering.md`、`openspec/config.yaml` 一律写「唯一实现基线 V3.6」；`tests/registry.yaml` 的
`release_lines` 标签写「V3.6 §1.4」；`schemas/tests/registry.schema.json` 的描述写「V3.6 §16」。

⇒ 同一仓库内对「唯一基线是哪一版」存在**两种互相矛盾的陈述**，且**没有任何 ADR 裁决过 V3.6 → V3.7**。
本 ADR 处置这一不一致。

## 证据（本轮实测，先读后判）

**规模**：`V3.6.md` = 1413 行；`V3.7.md` = 1875 行。

**V3.7 前 1413 行相对 V3.6 的差异**（`diff`，共 8 个 hunk，全部为 **1:1 行替换**）：

| hunk | 位置 | V3.7 相对 V3.6 改了什么 |
|---|---|---|
| `1c1` | 标题 | 版本号 V3.6 → V3.7 |
| `3c3` | 副标题 | 日期 2026-10-08 → 2026-10-09；「在 V3.6 基础上新增开放平台与技能市场」 |
| `29c29` | §0.2 | 增补「开放平台与技能市场见第三部分（第 33–38 章）；V3.7 的版本说明与汇总索引见第 39 章」 |
| `56c56` | §1.4 | R1 行增补「声明式技能与官方只读技能、市场客户端」与验收 `M01–M07、M13–M16、M18` |
| `151c151` | §4.1 | 增补「开放平台相关契约（Draft，自 R1 起逐步冻结）：skill.invoke、data.query、data.subscribe、device.sense、device.control、market.catalog、asr.transcribe」 |
| `174c174` | §4.5 | 首发形态增补「声明式技能与接入器（无自有代码）」 |
| `624c624` | §16 | 增补「扩展与市场相关的验收 M01–M18 见 37.5 与 38.10」 |
| `698c698` | §17.3 | R1 行增补「声明式技能与市场客户端」与 M 系列 |

**全部为增量补充，无一处删除或改写既有效力条款。**

**V3.7 第 1414–1875 行（462 行）**是**新增第三部分**：§33 开放平台总览、§34 数据源/系统/设备接入、
§35 技能规范、§36 技能市场与分发、§37 场景适配与开发者体验与路线、§38 市场开放接口与客户端适配、
§39 V3.7 版本说明与汇总索引。

**关键性质：行号完全对齐。** 8 处差异都是 1:1 行替换，故 V3.6 与 V3.7 在 1–1413 行的**行号一一对应**。
仓库大量以 `§NNN`（行号）形式给出的引用因此在两版解析**一致**——实测：

```
sed -n '655p'       两版均为 | B01 | 知识质量：100 题集达到第 6 章门槛 | R1 |
sed -n '1328,1329p' 两版均为 | P01 | 平台生命周期：… | R2 | 与 | O04 | 安装与设备：… | R1 |
```

## 决策

### 决策 1：V3.7 升为唯一实现基线

V3.7 是 V3.6 的**严格超集**（证据：内容只增不改，无删除、无改写）。升基线**不损失任何既有条款**，
且使文档陈述与 ADR 0034–0039 的既有事实一致。

- `docs/design.md` 指向 V3.7；
- `README.md`、`docs/development-plan.md`、`AGENTS.md`、`docs/engineering.md`、`openspec/config.yaml` 同步；
- V3.6 降为**历史参考**（与 V3.5 同等处置）：仅追溯，不自行拼接旧条款覆盖 V3.7。

### 决策 2：ADR 0015 的决策 2 与决策 3 继续有效

ADR 0015 有三项决策，其中**只有决策 1（V3.6 升基线）**由本 ADR 替代：

- **决策 2（勘误与裁决清单）继续有效**——`release_line` 独立字段、`provisional` 预冻结中间态、
  值守租约 600ms 撤权初值、X01 门槛统一到 M0b、决策日志缓冲满时丢遥测不丢关键提交等裁决，
  都是**内容层**裁决，与「基线是 V3.6 还是 V3.7」无关。
- **决策 3（命名保持 `nai-*`）继续有效**。

ADR 0015 **保留原文不改写**，仅在末尾追加「被 ADR 0042 替代」指针（历史记录追加勘误，不就地改写）。

### 决策 3：`§NNN` 行号引用只改版本前辍

因两版行号对齐，`§NNN` 的**数字不作变更**；只把**活文档**里的版本前辍从 `V3.6 §X` 订正为 `V3.7 §X`。
历史记录（`docs/reports/`、`docs/adr/`、`openspec/changes/archive/`）中的 `V3.6` 一律保留原文——
那些是点时刻快照，当时引用的就是 V3.6。

`tests/registry.yaml` 的 `release_lines` 标签从「V3.6 §1.4」改为「V3.7 §1.4」；
`schemas/tests/registry.schema.json` 的描述从「V3.6 §16」改为「V3.7 §16」；
`xtask/src/registry.rs` 的成功输出串从「V3.6 acceptance」改为「V3.7 acceptance」（这是**机器可见的当前状态断言**，
必须随基线变；`tests/registry.test.mjs` 的覆盖注释同步）。

**代码 doc-comment 中的版本前辍不在本次订正范围。** 例如 `crates/nai-kernel/src/duty.rs` 的 `V3.6 §21.4`、
`crates/nai-storage/src/decision.rs` 的 `V3.6 §5.8`、以及大量 `V3.5 §NN.N`。理由有二：① **先例一致**——ADR 0015
把基线从 V3.5 升到 V3.6 时同样未改动它们，仓库里至今留存大量 `V3.5 §…`；② 它们记录的是**该代码撰写时**所依据的
设计引用，属点时刻信息（与历史报告同理）。且因 V3.7 与 V3.6 行号对齐，这些引用在 V3.7 中**解析结果不变**，
不构成错误引用。

### 决策 4：本 ADR 明确不改什么

- **不改任何 acceptance `status`**：registry 48 项状态原样（tested 10 / implemented 27 / blocked 5 /
  pending 3 / not_applicable 3）。
- **不代劳 M01–M18 的登记**。本 ADR 只裁决「基线是哪一版」，**不追认** V3.7 第三部分已进入验收范围：
  V3.7 §39.5 与 §16 前导都写「每条验收在 `tests/registry.yaml` 中登记」，但 **M01–M18 至今一条都没登记**
  （实测 `grep -cE '\{id: "M[0-9]' tests/registry.yaml` 为 `0`），工作项 W26–W40 未开始。
  该缺口**单独处置**——要么登记为 `pending`（承认在册、未开始），要么另立 ADR 显式声明「第三部分不在当前
  release 范围」。**本 ADR 不选边，以免用基线切换掩盖一个独立的登记缺口。**

## 后果与实施

- 活文档的基线指针与 §-引用统一为 V3.7；全仓活文档不再出现 `V3.6`（历史记录除外）。
- 后续开发按 V3.7 切片发布线与 `tests/registry.yaml` 验收推进，D 编号仍只供追溯。
- **遗留（不在本 ADR 范围）**：
  1. **M01–M18 未登记**、V3.7 第三部分（第 33–38 章）**零实现、零登记**、W26–W40 未开始；
  2. `apps/` 缺设计列出的 `platform-bridge`(C++)、`control-server`、`update-helper`、`remote-web`；
  3. `crates/conformance` 未建为独立 crate；
  4. S08 的 SBOM 尚无规格。
