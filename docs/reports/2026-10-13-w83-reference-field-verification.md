# W83 · 引用字段的验证范围（触发条件 MUST 只由**字段存在**决定）

- 日期：2026-10-13
- 基线：V3.10（`docs/直播数字人 + 智能体系统技术方案 V3.10.md`）
- 裁定：[ADR 0061](../adr/0061-reference-field-verification.md)
- 变更：`openspec/changes/archive/2026-10-13-w83-reference-field-verification/`

## 一、判断与范围

W80 给待裁定台账确立了**一条**纪律，并把它写进了 `CITATION_FIELDS` 的注释：

> **扫描按「字段是否存在」触发，MUST NOT 按处置触发**——否则「注解型的字段」会被漏掉。

**但它只落到了 `§NNN` 这一侧。** 台账里的引用有**两类**：一类是 `§NNN`（`CITATION_FIELDS` 的七个
字段），一类是**登记 id**（`precondition`，`ID_LIST_FIELDS` 的**唯一**成员）。后者的校验写成：

```js
if (entry.disposition === 'needs_precondition' && Array.isArray(entry.precondition)) { … }
```

触发条件是**处置**——正是 W80 明令禁止的形状，只是它当时只看了一侧。

**范围**：只改 `scripts/contracts/pending-decisions.mjs` 与 `tests/registry.test.mjs`；
**不改基线正文、不改 `tests/registry.yaml`、不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边**。

## 二、审实

**审实 1（病根）**：`ID_LIST_FIELDS` 是一个类别声明，它存在的意义就是声明「`precondition` 的引用
**由 id 检查负责**」。若那个检查只在一种处置下运行，这个声明就是**条件性**的：字段一旦出现在别的
处置上，类别表说「它有主」，实际**没有任何人管它**（W73 的假契约同族：一句「SHALL 被校验」指出的
执行体，只在某些输入上才执行）。

**审实 2（探针，数据侧）**：只用公开 API、不改被测文件，用**真实注册表**：

```
F0 基线错误数 = 0                        ← 基线为绿

=== P1 PD-01 (needs_external) 加 precondition=['W99'] ===          （无新增错误）
=== P1b PD-01 (needs_external) 加 precondition=['H99'] ===         （无新增错误）
=== P2 PD-25 (answered_by_baseline) 加 precondition=['不是 id'] === （无新增错误）
=== P2b PD-09 (decidable_in_repo) 加 precondition=['W99'] ===      （无新增错误）

=== P3 对照 PD-09 (decidable_in_repo) 加 closure='§99.9' ===
  + PD-09: closure references §99.9, which does not exist in the baseline
```

**P3 是对照组**：同在一条 `decidable_in_repo` 条目上，`closure` 里的坏 `§` **被抓**、`precondition`
里的坏 id **不被抓**——**同一个洞的两半，只有一半被 W80 补上**。

**审实 3（数据分布）**：`precondition` 实测**只**出现在 4 条 `needs_precondition` 条目上
（`PD-23`：`["H13","H14"]`；`PD-26`：`["H05","H06","H16"]`；`PD-27`：`["H07","H08","H16"]`；
`PD-30`：`["W61"]`）。⇒ 把触发条件从「按处置」改宽为「按字段存在」后，**报错集合不变**：
放宽不会让任何既有数据变红（这正是「本轮不引入红」的判据）。

**审实 4（对照：`§` 侧确实是无条件的）**：`CITATION_FIELDS` 的扫描写在
`for (const field of CITATION_FIELDS)` 里、循环体**不含 `disposition`** ⇒ 同一条
`decidable_in_repo` 条目上 `closure='§99.9'` 会被抓。两类的**唯一差异**就是触发条件，别处一字不差。

**审实 5（`disposition` 该管什么）**：`disposition` 决定的是**「哪个字段是必填的」**
（`REQUIRED_FIELD`）——这是一条**预设**断言（这条处置 SHALL 带那个字段），按处置触发**正确**。
本轮修的是另一条断言——**「引用可解析」**（内容断言），它按处置触发**错误**。两者 MUST 分开
（W66：不合并不同维度）。

## 三、裁定

见 [ADR 0061](../adr/0061-reference-field-verification.md)。五条决策：

1. **`REFERENCE_FIELDS` 逐字写死**（八个：七个引用字段 ＋ `precondition`）——引用扫描域的**单一来源**；
   MUST NOT 由 `[...CITATION_FIELDS, ...ID_LIST_FIELDS]` 派生（派生出的「并集 == 它」是**恒真式**）。
2. **扫描循环由 `REFERENCE_FIELDS` 驱动，触发条件里不出现 `disposition`**——「字段是否存在」是唯一的触发。
3. **错误措辞「条目 + 字段 + 值」三件齐**，字段名由 `${field}` 生成（写死的字面量在新增字段的那一刻**说谎**）。
4. **只拒「不可解析的引用」，不拒「放错位置的引用」**——假阳性与假阴性不对称 ⇒ 永远选安全方向。
5. **`shapeErrors()` 双向具名断言**：`REFERENCE_FIELDS` ↔ `CITATION_FIELDS ∪ ID_LIST_FIELDS`。

## 四、实现

- `scripts/contracts/pending-decisions.mjs`：新增并导出 `REFERENCE_FIELDS`；条目循环里把原「§ 扫描」
  与「`precondition` id 校验」合并为**一个**按 `REFERENCE_FIELDS` 遍历的循环，按类别分派到两个解析器；
  `shapeErrors()` 增双向断言。
- `tests/registry.test.mjs`：**32 → 34**（import 加 `REFERENCE_FIELDS`）。

## 五、非空洞注入（F0 基线 + F1–F4 数据级 + F5–F8 表级）

```
F0 基线错误数 = 0（期望 0）

### F1 needs_external + precondition=[W99]                 PASS
   + PD-01: precondition W99 is not a work item named by the baseline
### F2 answered_by_baseline + precondition=[H99]           PASS
   + PD-25: precondition H99 is not a registry acceptance id
### F3 decidable_in_repo + precondition=[不是 id]          PASS
   + PD-09: precondition 不是 id is neither a registry acceptance id nor a baseline work item
### F4（反向）needs_external + 合法 precondition            PASS（不开火）
### F5 REFERENCE_FIELDS 去掉 closure                       PASS
   + field closure is owned by a check category but is not a reference field — the reference scan would skip it
### F6 REFERENCE_FIELDS 加幽灵字段 rationale               PASS
   + reference field rationale is declared but no check category owns it — its references would be resolved by nobody
### F7（反向）REFERENCE_FIELDS 顺序打乱                     PASS（不开火）
### F8（反向）还原后 shapeErrors                            PASS（不开火）；还原后基线错误数 = 0

=== 开火/通过合计: 8 / 8 ===
=== 还原核对: true ===
```

**F1–F3 三种坏值走三条不同分支**（作业项 / 验收 id / 两者都不是），都失败并报**条目 + 字段 + 值**；
三条用例的宿主条目**不缺**它自己的必填字段 ⇒ 证明抓住它们的是**引用解析检查**，不是「必填字段」那条。
**F4 是反向注入**：合法 id 出现在非专属处置上 **MUST NOT 红**（只拒不可解析、不拒放错位置）。
**F7／F8 是反向注入**：声明集顺序不敏感、还原后基线仍绿（边界没收过紧、本轮不引入红）。
表级注入用**临时模块副本**（该文件无 import ⇒ 可独立 import，符合既有做法）。

## 六、验证

### 6.1 定向测试

```
node --test tests/registry.test.mjs            →  34/34 pass   （32 → 34）
node --test tests/contracts/schema.test.mjs    →  17/17 pass
node .w83_inject.mjs                           →  F0 + F1–F8 共 8 项，全 PASS
```

### 6.2 门控

```
cargo xtask check --profile full   →  EXIT=0
```

实测 **91 个测试目标（73 `Running` + 18 `Doc-tests`）、860 passed / 0 failed**。
口径以 `grep -c '^test result:' <门控日志>` 定准（**每个目标恰好一行，没有自由度**）；
`^     Running ` 74 行 = **1 包装行**（cargo 自己打印的 `Running \`…nai-xtask.exe check --profile full\``，
**不是测试目标**）+ 73 真实目标。本轮**零 Rust 改动**，`passed` 与 W71–W82 逐项相同 ⇒「数字不变」
正是「未误改别处」的旁证。日志里可见本轮两条用例在门控内执行：
`ok 24 - an id reference must resolve wherever its field appears, not only under its own disposition`、
`ok 25 - the reference scan domain is a declared single source over every reference category`
（Node 用例**不**计入 91/860，那组数只来自 `cargo test`）。

### 6.3 两条 validate

```
openspec validate --all --strict              →  43 passed / 0 failed
openspec validate --all --strict --archived   →  81 passed / 0 failed   （80 → 81）
```

另对**活动变更**单独跑 `openspec validate w83-reference-field-verification --strict` ⇒ `valid`
（归档时 CLI 对 `proposal.md` 的 Why 段给了一条**非阻塞**警告「超过 1000 字符」；
归档**后**的 `--all --strict --archived` 仍为 **0 failed** ⇒ 该警告不使严格校验失败，如实登记）。


## 七、CI 复核

**run `38331933294`（`head_sha = 9985ae6c3b…`，与本轮推送 SHA 一致）= `success`**（run_number **96**，双平台绿）。

观测方式沿用既有纪律：**单次低频查询**（**不轮询**），每次带**唯一查询参数**破坏代理缓存、
`Cache-Control: no-cache`，并**回读 `head_sha` 与本次推送比对**；`curl` 带 `--tlsv1.2 --tls-max 1.2`
（schannel 与 TLS 1.3 握手失败，W77 实测）。本轮共 3 次列表查询：`in_progress`（推送后）
→ **被拒**（匿名配额打满）→ `success`（**配额重置后单次**读到，updated `01:36:27Z`）。
同一列表查询顺带证实上一轮 run `38330257112`（`head_sha = be80249a7c`，W82 回填）也是 `success`。

**流量事故（如实登记）**：本轮观测途中匿名配额打满（`/rate_limit` 显示 `remaining=0`、`reset=1791855656`）。
其症状与登记完全一致：响应体**不含 `status`**、`total_count=undefined`，正文为
`API rate limit exceeded for …` ——**看着像 run 状态异常，实为限流**。处置遵守纪律：**绝不轮询**，
读出 `reset`（UTC `01:40:56Z`）后**等待重置再做单次查询**（`/rate_limit` 本身不计数）。

**按观测轮终止约定**：本条回填提交会触发一个新的 run，**该 run 不再回填**——否则无限递推。

## 八、遗留

1. **仍不判语义**：脚本只证「引用的 id 存在」，**不证该前置真的是这条裁定的前提**（沿用 ADR 0054 决策 2）。
2. **id 清单目前只有一个成员**：若将来需要「只要 `precondition` 出现在某处置上就禁止」这类**位置**约束，
   那是另一条**预设**断言，MUST NOT 与本轮的可解析性检查合并（ADR 0061 遗留 2）。
3. **`note`／`closure` 等的处置归属不校验**（沿用 ADR 0058 遗留 2 / 0059 遗留 2 / 0060 遗留 2）。
4. **`question` 从此必须「引用可解析」**（沿用 ADR 0059 遗留 4）。
5. **§56.2 归档边界仍未写死**（W78 §八第 8 / W79 §八第 6 / W80 §八第 4 / W81 §八第 5 / W82 §八第 5 未变）：
   「上一轮已发布的归档」与「本轮刚归档、尚未入库」是否同一处置，仍需一次升基线轮次写死。
6. **`CAPABILITY_PACK_IDS` 仍是手写数组**（W78–W82 遗留未变；ADR 0056 遗留 4 已给理由）。
7. harness 轨道 **H18 与 B07** 仍 `pending`；W76 台账的 5 条 `decidable_in_repo`
   （`PD-09/22/24/31/34`）仍只登记了推荐——**推荐的落地仍未发生**。
8. `cargo xtask schema --check` 本机仍跑不了（宿主对 node 派生任何子进程的 `EBUSY`）。
   本轮未改任何 schema 与生成器输入 ⇒ 零漂移由结构性理由 + CI 独立步骤兜底，**未写成「已通过」**。

## 九、方法论教训

1. **纪律被写下来 ≠ 纪律被应用到每一处。** W80 的一般结论（「扫描按字段存在触发」）**原样写在注释里**，
   而它旁边的 `ID_LIST_FIELDS` 仍是按处置触发的。**一条规则的正例（`§` 侧）会让人以为它已经全局成立**
   ——所谓「已经修好了」往往只是「修在了我看见的那一处」。
2. **触发条件也要与数据无关。** 一份**完整**的清单，配上一个**条件性**的触发，覆盖仍然是条件性的。
   ⇒ 凡「某个集合的每个成员都被检查」这类断言，MUST 问：**触发条件依赖什么？** 依赖了「数据当前
   的状态」（处置、位置、顺序）就是不安全的。
3. **「预设」与「内容」 MUST 分开。** 「这条处置必带那个字段」是**预设**（按处置触发正确）；
   「这个字段的每个引用都解析得开」是**内容**（按处置触发错误）。两者混在一个 `if` 里，
   就会让后者继承前者的条件性。
4. **写死的字面量会在扩展时说话。** 原措辞里「precondition」是字面量；改由 `${field}` 生成后，
   新增一个 id 清单字段时**自动正确**——写死的字面量在那一刻会**说谎**（与 W79「措辞把定位引向错误
   方向」同族，只是方向相反：这次是**漏报**的措辞）。
5. **假阳性与假阴性不对称 ⇒ 只拒不可解析、不拒放错位置。** 「一个可解析但位置奇怪的前置」不虚假；
   拒绝它会把合法数据判红。判据的输入 MUST 等于被断言的对象：这里断言的是「引用可解析」。
6. **「不该红时不红」与「证明洞是真的」要成对**（W79–W83）：F4（合法 id 在非专属处置上不红）、
   F7（顺序不敏感）、F8（还原后基线绿）、F0（基线绿）——都在证**边界没收过紧 / 本轮不引入红**。
