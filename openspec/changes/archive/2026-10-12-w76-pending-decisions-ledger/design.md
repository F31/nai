# 设计（w76）

## 0. 核实结论先行

四章待裁定项是一条**累积链**：§39.7 沿用 §18.2 全部、§48.7 沿用 §39.7 全部、§57.5 沿用 §48.7 全部。
去重后的唯一集合是 **34 条**，分章的新增数为 §18.2 **10** / §39.7 新增 **10** / §48.7 新增 **11** /
§57.5 新增 **3**。

逐条核实后有两条关键发现：

1. **4 条已被基线自身回答**（不是遗漏，是「登记时以为待决」）：
   `sampling` 可接受场景（§46.2 给了默认禁用 + 三条件）、上下文利用率观测线（§41.4 判「本轮不设」）、
   `a2a` 契约名是否设立（§46.5 给了「不新增」+ 三条理由）、能力包打包粒度（§51.3 已是三行独立包）。
   ⇒ 它们的正确处置是**已答性核实并关闭**，与 W67 把「『通过 §47.2 的回归』是全六类还是相关类」
   核实为**不是偏差**同型。
2. **20 条是外部事实**（§52.1 的三条命题、市场项目约定、写实模型与目标机、平台/供应商资料），
   **5 条仓库内可裁**、**4 条需具名前置**、**1 条明确在方案范围外**（商务定价）。

## 1. 决策：五态处置，每态一个必填字段

用**封闭枚举**而不是自由文本，理由与 `release_lines`／`status` 的处置一致：
枚举让「这条属于哪一类」可判定，`note` 只承担补充说明。

| 处置 | 必填字段 | 判据 |
| --- | --- | --- |
| `answered_by_baseline` | `basis` | `basis` 里出现的每个 `§NNN` MUST 真实存在于**当前基线文本** |
| `decidable_in_repo` | `recommendation` | 非空；且 MUST NOT 与某条 `answered_by_baseline` 重复主张 |
| `needs_precondition` | `precondition` | 非空列表；每个元素 MUST 是**真实存在**的验收 id 或工作项编号 `WNN` |
| `needs_external` | `closure` | 非空；写「取到什么算闭合」而不是「待外部」 |
| `out_of_scope` | `note` | 非空理由（基线自述范围外也要付理由） |

**为什么是「每态一个必填字段」**：W73/W74 的教训是**判据要直接说出它要的东西**。
一条 `needs_external` 若没有 `closure`，它和一条空注释没有区别；而「有 `closure`」是可判定的。

## 2. 依据必须指回真实章节

`answered_by_baseline` 的 `basis` 是**唯一一处「台账宣称基线已答」的地方**，因此它必须被拴住：

- 从 `baseline.text` 抽出全部章节号：`^#{2,3}\s+(\d+(?:\.\d+)?)\s` ⇒ 得到 `{0.1, 0.2, 2, 3, …, 58.6}`；
- 从 `basis` 抽出 `§(\d+(?:\.\d+)?)`，逐个要求落在上面的集合里。

⇒ **写一个不存在的 `§99` 会立刻失败**，这正是这条判据的非空洞性所在（注入 V1 验证）。

**为什么不比正文措辞**：措辞会随行数契约漂移，而「章节号存在」是稳定且足够强的下界——
它排除掉「凭印象编一个章节」。语义是否真的回答了该问题，由 ADR 与报告承载，**不由脚本假装判定**。

## 3. 覆盖性：逐章写死，不靠台账自证

```
CHAPTER_COUNTS = [['§18.2', 10], ['§39.7', 10], ['§48.7', 11], ['§57.5', 3]]   // 新增数
TOTAL = 34
```

- 台账每条有 `introduced_in`（首次出现处）与 `also_in`（复述处）；
- 断言 `introduced_in` 的分布**逐章等于**上表，且总数 = 34。

**这是对 W66 教训的直接应用**：只断言「有 34 条」对「内容被改小」是瞎的——实测悄悄删一条必需字段，
全部测试照样绿。故每条**必填字段的存在性**由 §1 的规则单独覆盖，而**数量**由本表覆盖，两者不互相顶替。

## 4. 依赖方向与接线

- 新增 `scripts/contracts/pending-decisions.mjs`：零依赖（只用 `node:fs/promises` 与既有的 `yaml`），
  导出 `DISPOSITIONS`、`CHAPTER_COUNTS`、`validatePendingDecisions(ledger, deps)`；
- **deps 显式注入**：`{baselineText, acceptanceIds, workItemIds}`。缺任一项时判据 MUST 报错——
  一条会因为入参缺失而恒真的断言比没有断言更坏（W72 对 `normalizeDeviations` 的原话）；
- 断言**并入 `tests/registry.test.mjs`**：该文件已由 `xtask/src/registry.rs::check` 通过
  `check --registry` 执行 ⇒ **`engineering_checks` 零改动**（W74/W75 的同一条处置）；
- 不新增 xtask 子命令、不新增证据目录（§30.1 的八个目录是闭合集）。

## 5. 验证命令与环境边界

```
# 单跑（自包含，不需要先跑别的步骤）
node --test tests/registry.test.mjs

# 门控（本机可达）
source /c/Users/32791/.nai-msvc-env.sh
CARGO_TARGET_DIR=E:/nai-build-target CARGO_INCREMENTAL=0 \
  cargo xtask check --profile full
```

- **不需要 protoc、不需要编译产物**：本步只读 YAML/文本；
- **Windows 与 Linux 行为一致**（纯静态读文件，无平台分支）⇒ 非空洞性不依赖某一平台；
- `cargo xtask schema --check` 本机仍跑不了（宿主对「node 自旋 node」的 `EBUSY`），本轮的零漂移
  由「未改任何 schema 与生成器输入」＋ CI 的独立步骤兜底，**不写成「已通过」**。

## 6. 与既有机制的关系（不重复、不顶替）

| 既有机制 | 与台账的关系 |
| --- | --- |
| `baseline_deviations`（W72/W73） | 那是**已发生的偏离**台账（现在为空）；本台账是**未决的问题**。两者维度不同，**不合并** |
| `baseline.mjs` 的对账 | 它管「基线 §57.3 ↔ registry」；本台账管「基线待裁定章 ↔ 处置」。同构但对象不同 |
| `tests/registry.yaml` | 验收条目；台账不新增验收条目，**不动 registry** |

## 7. 附带修复：测试夹具的 bring-up 预算

**发现路径**：本轮第一次跑门控时 `cargo test --workspace` 在
`crates/nai-runtime/tests/common/mod.rs` 的**准备阶段**失败（`set_permission(...).unwrap()` →
`StorageError { code: DeadlineExceeded, reason: "operation stopped" }`）。核实：

| 观察 | 结论 |
| --- | --- |
| 本轮**零 `.rs` 改动**，且 W75 时本机门控绿 | **不是本轮引入** |
| 失败在 `set_permission(...).unwrap()`（夹具）而非断言 | 失败在**准备阶段**，不在被测路径上 |
| 单独跑该包**也红**，但失败的是**另一个**测试（`common/mod.rs:99` 而非 `:139`） | 失败点在**同一夹具的不同行** ⇒ 并发争用的 signature |
| `-- --test-threads=1` ⇒ **12/12 绿** | 并发争用确认（但 W70 §3.2 已否决「靠串行」） |

**根因**：夹具的 `operation()` 自造了 `Operation::new(Duration::from_secs(5))`，而 `open_store`／
`provision` 做的是 `Store::open`（**全新数据根上跑全链迁移**：六个 `BEGIN IMMEDIATE`／`COMMIT`）
＋建 workspace ＋批量授权 ＋存 profile。**这是 bring-up 的工作量，不是逐次调用的工作量。**

**产品侧早就把这件事写清了**——`crates/nai-runtime/src/composition.rs`：

> `SHORT_OPERATION` = 5 s：*the budget for a call that touches rows, **not** for a bring-up that runs
> the schema migration chain*；
> `BRINGUP_OPERATION` = 30 s：*Sized here once so the engine **cannot hold a second, drifting copy***。

⇒ **夹具持有的正是那句注释所禁止的「second, drifting copy」，而且它描述错了工作量。**
旁证：`nai-storage` 自己的夹具（`src/tests.rs:10`、`src/upgrade.rs:183`）用的就是 30 s ——
**只有 `nai-runtime` 的夹具是 5 s**。

**修法**：夹具复用 `nai_runtime::BRINGUP_OPERATION`（`lib.rs` 已重导出），`open_store`／`provision`
走新的 `bringup()`；`operation()`（5 s）保留给 `store_operation()` 与 `verify_chain` 这类**逐次调用**。

**为什么这不是 W70 §3.2 否决的那三种处置**：

- 「改成阻塞 lock」否决的是**改变语义**（把竞争换成无限等待）——这里没动任何同步语义；
- 「只在测试夹具里等」否决的是**用夹具掩盖产品缺陷**——但这一步是**前置准备**
  （`provision` 的文档注释：「Creates a workspace, grants the runtime permissions and stores the
  profile」），**产品代码在这里不被考察**；夹具此前是**分配了不足的预算**，不是把失败藏起来；
- 「`--test-threads=1`」被明确否决 —— 故验证**在默认并发下连跑 3 轮**。

**判定**：默认 test-threads 连跑 3 轮 `cargo test -p nai-runtime --test composition` → **3× 12/12 绿**
（各 38–39 s）。**不改任何产品代码。**

---

## 勘误（W76 CI 回填轮追加）

**本文档 §0 与 §3 的分章数写错。**

- §0 原文：「分章的新增数为 §18.2 **10** / §39.7 新增 **10** / §48.7 新增 **11** / §57.5 新增 **3**」；
- §3 原文：`CHAPTER_COUNTS = [['§18.2', 10], ['§39.7', 10], ['§48.7', 11], ['§57.5', 3]]   // 新增数`。

**实测真值**（`tests/pending-decisions.yaml` 逐条 `introduced_in` 统计，共 34 条）：
**§18.2 = 10 / §39.7 = 11 / §48.7 = 10 / §57.5 = 3** ⇒ 即 **§39.7 与 §48.7 两处的数字被互换**。

**旁证**：`scripts/contracts/pending-decisions.mjs` 的 `CHAPTER_COUNTS` 与
`docs/adr/0054-pending-decisions-ledger.md` 正文写的都是真值 10/11/10/3（两者一致，且门控绿）。
⇒ **错的只是本文档的这两处转述**；设计决策（§3「逐章写死，不靠台账自证」）本身不受影响。

**处置**：本文档位于 `openspec/changes/archive/`，按 §56.2 属**历史记录** ⇒ **原文保留 + 追加本段**，
不就地改写。相关登记见 `docs/reports/2026-10-12-w76-pending-decisions-ledger.md` §九第 6 条。
