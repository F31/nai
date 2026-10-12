# W73 报告：注册表形状契约接电与台账存活期

- 轮次：W73（2026-10-12）
- 变更：`openspec/changes/archive/2026-10-12-w73-registry-schema-and-ledger-lifetime/`
- 裁决：[ADR 0051](../adr/0051-registry-schema-wiring-and-ledger-lifetime.md)
- 相关：[ADR 0050](../adr/0050-baseline-registry-reconciliation.md)（W72 对账门控）、[ADR 0049](../adr/0049-design-baseline-v310.md)、[ADR 0047](../adr/0047-design-baseline-v38.md)

## 一、判断与范围

本轮的**出发点是 W72 遗留第 2 条**（§56.2「紧接着订正」与「跨轮次收录」的张力**未裁定**）。
**核实之后改了计划**：核实发现的问题比那条遗留更靠前。

**规格里写着「schema 校验 SHALL …」，而那句校验根本没跑。**

三条既有规格要求把结论挂在 schema 校验上：

| 位置 | 原文（节选） |
|---|---|
| `admission-registry` / `Single machine-readable admission registry` | 「schema 校验 SHALL 拒绝重复编号、未知 release_line/status、悬空映射与新增的旧 T/E/U 编号」 |
| `admission-registry` / `Release lines are declared…` | 场景「The new line is not registered in the schema」：MUST NOT 出现「注册表写得进去、schema 认不出来」的状态 |
| `admission-registry` / `The frozen baseline and the registry reconcile…`（W72 新增） | 台账每条的形状**只写在文档里**，无 schema 可依 |

**结论**：`tests/registry.yaml` 的形状一直只由手工检查把关，而**手工解析对未知键静默忽略**；
**W72 新增的 `baseline_deviations` 正是在这个空档里进去的**——它会令 schema 校验失败，
但没有任何执行体把注册表交给 schema。

**所以本轮不是「补一条断言」，而是把一条已经声明过、却从未生效的契约接上电。**
台账的存活期是同一件事的另一半：W72 把「已登记但未收录」变成可判定状态，
但**没有**限制它「可以存在多久」；本轮补上寿命。

**明确不改**：基线正文、`tests/registry.yaml` 的任何 `acceptance` 条目、Rust 行为、xtask 步骤数、生成物。

## 二、核实（三件事，全部先读后判）

### 2.1 该 schema 无人消费

`schemas/tests/registry.schema.json`：不在 `scripts/contracts/validation.mjs` 的 `sources`（列了 11 份）、
全仓无 `include_str!("…/schemas/tests/…")`、无任何 `*.mjs`/`*.rs` 读它。

**再实测一次它到底对不对**（把注册表解析成对象后直接喂给 Ajv）：

```
registry 顶层键：schema_version, release_lines, acceptance, mapping, baseline_deviations
schema 校验通过： false
  - (root) additionalProperties must NOT have additional properties
  错误总数： 1
```

**只差一处**：W72 新增的 `baseline_deviations` 未被声明；其余 **88** 条、**六**条发布线、映射段**全部通过**。
⇒ 它**不是**文档残留（`description` 随 W65/W71 更新过，写着「V3.10 §16 + §48.5 + §51.1 + §57.3 + ADR 0015 errata」），
它只是**从未接电**。**一次合法的往前推进一步，就把一个无人检查的契约变成了假契约。**

### 2.2 孤儿不止一个，而「有没有人读它」没有守卫

对 `schemas/**/*.schema.json` 逐个查消费点：

| 消费形态 | schema |
|---|---|
| Ajv 校验器清单（`validation.mjs` 的 `sources`） | `config/{runtime,provider,backup,binding-plan,avatar-2d}`、`plugin/{manifest,model,avatar,ui}`、`catalog/catalog`、`platform-matrix/platforms` |
| **只在 Rust 侧**（`include_str!`） | `config/upgrade-package`（`crates/nai-upgrade-core/src/manifest.rs`） |
| **无** | **`tests/registry`**、**`rag-golden/case-set`** |

`config/upgrade-package` 这一行顺带定下了守卫的口径：**只扫 Node 会把它误判成孤儿**
⇒ 「被消费」必须从**三条来源**派生（校验器清单 / Rust 的 `include_str!` / 代码里的路径字面量）。

### 2.3 「紧接着」在历史上从未按字面发生，且字面读法不可行

逐版读**基线正文本身**（不是报告里的话），把「偏差出现 → 被收录」列出来：

| 实例 | 偏差何时出现 | 何时收录 | 是否跳过升基线轮次 |
|---|---|---|---|
| 爬坡 / H18 的 E4 | W67（登记） | W71（V3.10） | **否**（W67 之后的下一次就是 W71） |
| B07 | W68（登记） | W71（V3.10） | **否**（W68 之后的下一次就是 W71） |
| A12–A14 | **W56**（见下） | W65（V3.9） | **否**（W56 之后的下一次就是 W65） |

**并订正 W72 报告的一处表述**（原文不改写，追加勘误段）：A12–A14 **不是**「跨轮次收录」。
W54/W55 新增 A12/A13/A14 时，**活索引 §48.5 还不存在**（§48 是 V3.8 的新章）；
当时存在的 `§39.5 验收总表索引` 是 **V3.7 的版本专属章节**（§56.2 情形③），其落后是**设计**。
偏差是**在 W56 出现的**——ADR 0047 新建的 V3.8 里，那份活的跨版本索引
**诞生时就落后于注册表**（A 行写 `A01–A08；A09–A11 补充`，而注册表此刻已有 A12–A14），
直到 W65 被收录，中间**九轮**没有任何记录写着「这里不一致」。
⇒ 它是**静默漂移**，不是「登记了但收录得晚」——**这两种失败不是一回事**。

⇒ 三条实例**全部在下一次升基线轮次被收录，零跳过**。字面「同一轮内」不可行，理由有三：

1. 基线被 ADR 批准后**不可变**，且就地订正有硬约束 **MUST 1:1 行替换** ⇒ 改 §57.3
   **只能发生在一次专门的升基线轮次里**，「登记之后紧接着订正」在实现轮次内**没有可执行的落点**；
2. **轮数是外部观察值**（本仓库已有同一纪律，见 §48.7 的阈值三件）：取 3／9／11 都无依据；
3. **升基线轮次是仓库内的离散事件**，且是唯一可改 §57.3 的时刻 ⇒ **锚事件，不锚计数**。

### 2.4 「schema 校验」这个不实前提在仓里还有几处副本（C6 教训）

不实陈述**从来不止一处**（C6 时曾散落四个活文档）⇒ 订正前先全仓扫描。本轮扫了四类载体：

| 载体 | 命中 | 处置 |
|---|---|---|
| 活规格 `openspec/specs/*/spec.md` | `admission-registry` 的 requirement 正文与场景各 1 处 | **本轮 MODIFIED**（改成「MUST 真的被执行」） |
| 活文档 `docs/*.md`、`AGENTS.md`、`README.md` | `development-plan.md:629` 指 **config/plugin 契约**的 schema 校验（那份**确实在**校验器清单里，属实）；其余全是 W73 自己的描述 | 无需订正 |
| 历史记录 `docs/adr/0042/0047/0048/0049`、`docs/reports/*` | 只陈述「schema 的**哪两片**位置要同改」（**形状**命题，属实） | 原文不改 |
| 历史记录 `openspec/changes/archive/2026-10-08-d15-*/design.md` | 「`tests/registry.yaml` … **由 `schemas/tests/registry.schema.json` 校验**」——**这句在写法上是不实的** | **不就地改写**（历史记录保留原文），在此具名登记 |

最后一行值得写清楚，因为它不能靠「历史记录保留原文」一句话糊过去：那句话陈述的是**设计意图**，
而设计意图此后数十轮**没有被任何执行体兑现**；**W73 把它接上电之后，这句话反而重新成立了**。
⇒ 处置是**在此登记该副本的时点**（W73 起为真，W73 之前为假），而不是去改一份历史设计文档——
改了会让人以为它当初是对的。**这与「全仓扫描」并不矛盾：扫描的目的是找到副本，处置的判据是载体类型。**

## 三、裁定（ADR 0051 的四条决策）

| # | 决策 | 一句话 |
|---|---|---|
| 1 | **注册表形状契约接电** | `tests/registry.yaml` MUST 满足其 schema；**只改 `description` 不算修复，删掉 schema 也不算**（三条既有规格要求以它为前提） |
| 2 | **形状层与语义层只有一份定义** | 两层的**三处重叠常量**（id 家族 pattern / 发布线枚举 / 状态枚举）MUST 逐字一致，不一致时**指名是哪一处**；重叠允许，漂移不允许 |
| 3 | **台账条目 MUST NOT 跨基线版本存活** | 条目加 `since_baseline`；跨版只有**收录**或**显式结转**（改版本 + 记 `carried_at` + 非空理由） |
| 4 | **`schemas/` 下每份 schema 要么被消费、要么具名声明** | 消费关系**派生**；例外 MUST 附非空理由；本轮例外恰一条 |

**决策 3 为什么写成「寿命」而不是「轮数上限」**：台账的**长度**可以靠沉默增长，**寿命**不能。
`since_baseline` 与当前基线版本不一致 ⇒ 失败并**同时给出该条目与两侧版本**。
它的实际成本**现在是零**（台账为空），它的作用在**下一次升基线**时才收账——
这正是我们要的形状：**把「沉默」变成「记录」**。

**决策 4 为什么是「具名例外」而不是「一律接线」**：判据不是「有没有 schema 文件」，
而是「**谁在判它**，以及**为什么是它**」。`rag-golden/case-set` 的执行体是
`xtask/src/golden.rs` 的 `parse_case_set`，它对每类缺陷产出**稳定 token**
（`unknown_field`／`field_missing`／`case_set_schema_unsupported`…），调用方据此定位；
再接一份 JSON Schema 会形成**同一形状的第二处判定**，而两份判定的失败**措辞不同**
⇒ 只会让「为什么被拒」有两个答案。

## 四、实现

| 文件 | 改动 |
|---|---|
| `schemas/tests/registry.schema.json` | 补 `baseline_deviations` 定义（`additionalProperties: false`、字段级形状）、列入根 `required`、`description` 标注「本 schema 由 `tests/registry.test.mjs` 执行，不是文档」 |
| `scripts/contracts/validation.mjs` | 导出 `AJV_SOURCES`（把「有哪些 schema」变成**单一清单**）；注册表入列 |
| `scripts/contracts/registry.mjs` | 导出 `ID_RE` 与 `STATUSES`（一致性断言需要） |
| `scripts/contracts/baseline.mjs` | `normalizeDeviations(raw, errors, version)`：`since_baseline`/`carried_at` 与跨版判据；`reconcile(registry, parsed, version)`：版本入参**必需**，缺失/非法**具名报错**（MUST NOT 静默跳过） |
| `tests/registry.test.mjs` | 新增 3 个用例（形状／一致性／寿命）；全部 `reconcile` 调用点经统一助手传入版本；**11 → 14** |
| `tests/contracts/schema.test.mjs` | 新增 1 个用例（消费守卫）；**12 → 13** |
| `tests/registry.yaml` | 台账段注释写明新形状与「跨版必须显式结转」；**台账仍为空** |

**为什么 `version` 是必需入参而不是可选**：可选参数会让这条判据在缺参时**恒真**——
而一条会因为入参缺失而恒真的断言，**比没有断言更坏**（它会提供虚假的保证）。故缺参⇒具名失败。

**门控路径不变**：`tests/registry.test.mjs` 与 `tests/contracts/schema.test.mjs` **都已在**
`xtask/src/lib.rs` 的 `engineering_checks` 内（`registry::check`、`schema::node`）
⇒ 新守卫自动进入 `portable` 与 `full`，**不需要新的 xtask 步骤**。

## 五、非空洞注入（各组都必须被抓住）

**本轮新增 27 组**（W72 的 21 组继续有效）：

| 组 | 数量 | 内容 | 期望 |
|---|---|---|---|
| A schema 形状 | **8** | 未知顶层键 / 未知条目字段 / 坏 id / 坏发布线 / 坏状态 / 空 `acceptance` / 少一个 D 键 / 缺台账键 | 全部失败，且**报在各自路径上**（`/acceptance/0/id`、`/acceptance/0/status`、`/mapping/d01_to_d14`…） |
| B 两层一致性 | **3** | 改小 `ID_RE` / 删一条发布线 / 加一个状态 | 失败并指明是哪一处（id 家族 pattern / 发布线枚举 / 状态枚举） |
| C 台账寿命 | **8** | 跨版存活 / `carried_at` 不晚于登记轮次 / 坏 `carried_at` / 缺 `since_baseline` / 缺版本入参 / 坏版本串 ⇒ 失败；**合法结转** / **相称条目** ⇒ 通过 | 见左 |
| D 消费守卫 | **8** | 删一项消费 / 给已消费的加例外 / 例外缺理由 / 例外指向不存在的文件 ⇒ 失败；**整行 `//` 注释里的路径不算消费**、块注释续行不算、workflow `#` 注释不算、代码里的路径仍算 ⇒ 通过 | 见左 |

**D 组第三、四条是本轮的自我修正**：守卫第一版把**自己文件里的字面量**算成了消费，
于是具名例外被判成「既被消费又是例外」⇒ 判据**自我满足**。
修法不是加特例，而是**把本文件排除在扫描之外**（并写明理由），再补一条
「**整行注释里的路径 MUST NOT 算消费**」——遮住最常见的形态（文档注释/块注释引用路径），
不去冒险解析行尾注释与字符串里的 `//`（`https://` 会因此被误切）。
**方向很重要**：行尾注释仍算消费，这个假阳性只会让孤儿**更难**被漏掉；反过来（注释算消费者）
会让守卫在**危险的方向**上变瞎——删掉真正的加载点、只留一句注释，孤儿照样判不出来。

**另外保留的守卫**：「扫描非空且命中 Rust 侧消费」——若扫描恒空，
「没有孤儿」就会因为**什么都没扫到**而恒真。实测消费报告：**14 份 schema / 13 份被消费 / 1 份具名例外**，
扫描 **240** 个代码文件。

## 六、验证

| 项目 | 结果 |
|---|---|
| `node --test tests/registry.test.mjs` | **14 / 14**（本轮 **11 → 14**） |
| `node --test tests/contracts/schema.test.mjs` | **13 / 13**（本轮 **12 → 13**） |
| 注入验证 | **27 组**（A 8 + B 3 + C 8 + D 8），各组都被对应守卫抓住 |
| 消费守卫实测 | 14 份 schema / 13 被消费 / 1 具名例外；扫描 240 个代码文件 |
| 差量需求正文长度 | 381 / 459 / 239 / 230 / 250 字符（全部 < 500；`specs/` 侧的硬限，归档前必量） |
| 不实前提的全仓副本扫描 | 四类载体各查一遍，**活文档侧零副本**；唯一未尽的一处在 `archive/2026-10-08-d15-*/design.md`（历史记录，具名登记见 §2.4） |
| 基线契约 | **不受影响**：本轮**不触碰**基线文档 |
| `cargo xtask check --profile full` | **EXIT=0**：目标 **91（73 `Running` + 18 `Doc-tests`）**、**858 passed / 0 failed**——与 **W72 逐项相同**（本轮零 Rust 改动 ⇒ 数字不变本身就是「未误改代码」的旁证；`Running` 行 74 中含 **1 条 cargo 包装行**，不计分） |
| 门控内的两个新守卫 | 同一次 `full` 日志里读到 `# pass 14 / # fail 0`（`registry.test.mjs`）与 `# pass 13 / # fail 0`（`schema.test.mjs`），以及 `admission registry check passed (V3.10 acceptance + D01-D14 mapping)` |
| `openspec validate --all --strict` | **43 passed / 0 failed**（W72 为 **42**） |
| `openspec validate --all --strict --archived` | **71 passed / 0 failed**（W72 为 **70**；每归档一次 +1） |

## 七、CI 复核（匿名观测）

推送后 `on: push` 自动触发；两平台（`ubuntu-24.04` / `windows-latest`）**跑同一条 `check --profile full`**。

| run | head | 结论 |
|---|---|---|
| [`38227339344`](https://github.com/F31/nai/actions/runs/38227339344) | `a219fb2`（与本轮推送 SHA 一致） | **双平台双绿**：`foundation (ubuntu-24.04)` **success**（**5m49s**）、`foundation (windows-latest)` **success**（**11m17s**）；run 总状态 **Success**、总时长 **11m22s**（两 job 计费时长合计 17m6s）。唯一非绿信号是两 job 各一条 Node.js 20 弃用 warning（`actions/checkout@v4` 等被强制跑在 Node 24 上，**非阻断**，与 W70–W72 同形） |

**观测方式与配额账**：推送后先读列表页确认 run 已注册（此刻 **In progress**），再等其跑完
（上次同类 run 耗时 13m0s ⇒ 挂后台等待而非轮询）。本机匿名配额此时**已打满**
（`/rate_limit`：`remaining 0`、`reset 09:31:20Z`，而观测时刻 **09:17Z**）⇒ 三次观测**全部**改走
WebFetch 读服务端页面（**不吃本机配额**），并按 15 分钟同 URL 缓存的性质**换查询串**
（`?w73backfill=1` → `?j=windows1` → `?u=1`）。三次回读的 `head_sha` 均为 `a219fb2…`，
与本次推送**一致** ⇒ 结论不是过期缓存的旧 run。

**一个顺带确认的事实**：CI 的 windows job 步骤里有
`Run pnpm exec openspec validate --archived --no-interactive` ⇒ 本轮归档后的**已归档变更口径**在 CI 上也被跑到，
不只是本机跑过一次。

**这次绿说明什么、不说明什么**：它证明本轮的 `full` 门控在两平台都通过——
特别是**新增的 Node 守卫在 Linux 侧也绿**（消费守卫要扫全仓 `.rs`/`.mjs`，这条路径只在 CI 上被非 Windows 环境完整跑到；
`tests/registry.test.mjs` 14 例、`tests/contracts/schema.test.mjs` 13 例在两个 job 里都跑到）。
它**不**说明此后不会出现间歇失败；本轮零 Rust／零测试目标改动，与那种红本就无关；
本轮立论**不依赖**这次绿，而依赖可复现的本机证据（两个测试文件的用例数 + 27 组注入 + 消费报告 + 两条 validate 口径）。

（按观测轮终止约定，**回填 CI 结论的那次提交其自身 run 不再回填**。）

## 八、未做与遗留

1. **语义类漂移仍只能靠人读**（沿用 ADR 0050 遗留 1）：本轮判的是台账的**寿命**，不是理由的**内容**——
   「这条理由是否成立」只能读出来。
2. **注册表之外的形状漂移**：本轮只给 `tests/registry.yaml` 接电。`schemas/` 之外的数据文件
   （如 `docs/reports/evidence/*`）没有形状契约，未动。
   **消费守卫有一个已知盲点（如实登记）**：`scripts/contracts/generate.mjs` 用**模板路径**
   （`schemas/${source}.schema.json`）消费 10 份 schema，其具体取值抓不出来 ⇒
   「仅由模板消费」的 schema 会被**误报为孤儿**。今天不漏判（那 10 份是 `AJV_SOURCES` 的子集），
   且误报方向**安全**（多报一次，逼人做决定）；将来真出现这种形态时，做法是把该清单做成
   第四条**派生来源**，而不是给它加例外。
3. **`§56.2` 自身的两处张力，仍未裁定**：① 它把 `§56` 列入「该版专属的版本章节」（情形③），
   而 §56.2 的内容是**一般规则**（V3.10 的 E5 正是按一般规则去改它的），两种读法都成立；
   ② 前言「紧接着订正」的字面读法不可行（本轮已论证），但**基线正文未改**。
   ⇒ 两者都需要一次**基线修订轮次**裁定；本轮只把**可判定的一半**（台账寿命）落地，
   并把它记进 `spec-driven-delivery` 与 `admission-registry` 的规格。
4. **`rag-golden/case-set` 的具名例外需要人来维持它的理由**：守卫只能判「有没有理由」，
   判不了「理由是否仍然成立」——若某天执行体从 `golden.rs` 换掉，这条例外要靠读。
5. **H18、B07 仍为 `pending`**；§57.5 与 §48.7 的待决策项（能力包打包粒度、MCP/A2A 协议版本与认证、
   阈值三件等）沿用 W71/W72 的登记，未动。

## 九、方法论教训

1. **规格里的「SHALL」是一句承诺，不是一句事实。** 本轮抓到的是「规格写着 schema 校验、schema 从未被读」——
   `grep` 三处规格要求只花一分钟，而它推翻了一整轮的计划。**动手前先问：这句话今天是靠什么成立的？**
2. **守卫要在「危险的方向」上不瞎。** 消费守卫第一版把自己文件里的字面量算成消费，
   于是判据**自我满足**；这类「守卫自我满足」比守卫不存在更坏。修法是**排除 + 说明理由**，
   而不是给样本开特例。**凡是「从全仓文本派生」的守卫，都要先问：它会不会读到自己？**
3. **假阳性与假阴性不对称。** 注释算消费是**假阳性**（把孤儿藏起来，假绿），
   代码算消费而注释不算只是**假阴性**（多报一次，逼人做决定）。**在两者之间选，永远选安全的那个。**
4. **「可判定」还要加一维：时效。** W72 把「已登记但未收录」变可判定，但它没说「可以存在多久」。
   **一个可以无限期存在的待办，等于一个不会告警的待办。** 加寿命的办法不是定一个轮数上限
   （那是外部观察值），而是锚在一个**仓库内的离散事件**上（这里是「升基线」）。
5. **锚点要选「唯一可行时刻」。** 之所以能锚「下一次升基线」，是因为基线不可变 + 就地订正 MUST 1:1
   行替换 ⇒ **改 §57.3 只可能发生在升基线轮次**。**先找那个「事情只能在这时做」的时刻，再定判据。**
6. **历史要逐版读正文，不能读后人的总结。** W72 把 A12–A14 记成「跨轮次收录」，
   而逐版读出来的事实是「W56 的活索引诞生即落后、W65 收录」——**静默漂移**与「登记了但收录得晚」
   是两种不同的失败，混为一谈会让结论（以及据此定的规则）偏掉。
7. **「只在某一侧被消费」是真实形态，不要只扫自己习惯的那一侧。**
   `config/upgrade-package` 只在 Rust 的 `include_str!` 里被消费；
   只扫 Node 会把一份**有执行体**的 schema 判成孤儿，进而诱导出「给它加个例外」的错误动作。
