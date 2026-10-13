# W84 · 测试文件的可达性闭世界（在册测试文件 MUST 被某个登记站点覆盖）

- 日期：2026-10-13
- 基线：V3.10（`docs/直播数字人 + 智能体系统技术方案 V3.10.md`）
- 裁定：[ADR 0062](../adr/0062-test-reachability-closure.md)
- 变更：`openspec/changes/archive/2026-10-13-w84-test-reachability-closure/`

## 一、判断与范围

Node 测试是靠**逐条列举**接上电的。`xtask/src/lib.rs` 里手写：

```rust
schema::node(root, &["--test", "tests/contracts/schema.test.mjs"])?;
schema::node(root, &["--test", "tests/platform/platform-matrix.test.mjs"])?;
```

`xtask/src/registry.rs` 里手写 `tests/registry.test.mjs`；此外还有**两个 glob**
（`package.json` 的 `tests/ui/*.test.mjs`、`apps/desktop/package.json` 的 `tests/*.test.mjs`）。
**这份机制的盲区恰好是它的补集**：一个没有任何站点指向的测试文件，谁都注意不到它。

**范围**：新增 `scripts/contracts/test-reachability.mjs`、改 `tests/registry.test.mjs`；
**不改基线正文、不改 `tests/registry.yaml`、不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边**。

## 二、审实

**审实 1（洞是真的）——探针实测，非推断。** 写一个内容只有 `assert.fail('W84 PROBE: this file never
ran …')` 的测试文件，放进 `tests/kernel/w84-probe.test.mjs`，`git add` 使它进入 `.git/index`，
然后跑 `cargo xtask check --profile portable`：

```
EXIT=0
test result 行数 = 91        （73 Running 目标 + 18 Doc-tests）
passed = 860   failed = 0
grep -c "w84 probe" .w84_probe2.log = 0     ← 探针的用例名在日志里零出现
```

**一个含必定失败断言的测试文件，能被完整门控完全忽略，且没有任何信号。**
这不是「未登记 = 不跑」（那至少是一种可解释的行为），而是**静默的不跑**——
它让「仓库里所有测试都在跑」这句话整体不可信。

> **一次被正确排除的干扰**：首次探针运行曾报红，失败在 `nai-runtime --test composition`
> （`DeadlineExceeded / "operation stopped"`，`composition.rs:225` 的 bootstrap 期限）。
> 按既有判据「**单目标能过 ⇒ 非代码缺陷**」：单跑该目标通过 ⇒ 宿主瞬态。
> 更关键的一步是**看探针的用例名在日志里出现过几次**——**0 次** ⇒ 那次红与探针无关。
> 干扰被排除后才重跑取绿，才把「探针被静默忽略」立成结论。

**审实 2（反面假设先被排除）**：起初怀疑 `tests/ui/ui-layer-declaration.test.mjs` 是孤儿
（全仓只有一份报告提过它）。核实后**不成立**——`tests/ui/*.test.mjs` 是**通配**展开，4 个文件都跑。
记这一笔是因为它给出了两条并行的教训：**「被引用」不等于「被运行」**，而**「没被逐条列举」也不等于
「不运行」**——两者都要读到实现（`frontend-gate.mjs` 的 glob 展开）才能裁定。

**审实 3（同类登记）**：「按清单做」的守卫，盲区是清单的补集——W80–W83 修的是待裁定台账的**三层**，
本轮修的是**门控接线**这一层，形状完全同族：

| 轮次 | 「清单」 | 「清单的补集」 |
|---|---|---|
| W80 | 三个 `sectionRefs(...)` 手写字段 | `recommendation`/`closure`/`note`（17 条引用漏扫） |
| W81 | `CITATION_FIELDS` | 清单**之外**的新字段 |
| W82 | 条目的键集 | 信**封**（顶层）的键集 |
| W83 | `§` 一侧的触发规则 | id 一侧仍**按处置**触发 |
| **W84** | **逐条列举 + 两个 glob** | **没有任何站点指向的测试文件** |

**审实 4（登记站点逐字采集）**：七个站点的 `host`／`marker`／层级逐一读自文件（非猜测）：

| id | host | marker | base | tier |
|---|---|---|---|---|
| `xtask-registry` | `xtask/src/registry.rs` | `tests/registry.test.mjs` | `.` | portable |
| `xtask-engineering-schema` | `xtask/src/lib.rs` | `tests/contracts/schema.test.mjs` | `.` | portable |
| `xtask-engineering-platform` | `xtask/src/lib.rs` | `tests/platform/platform-matrix.test.mjs` | `.` | portable |
| `xtask-contracts-suite` | `xtask/src/schema.rs` | `tests/contracts/descriptor.test.mjs` | `.` | ci |
| `ci-contract-suite` | `.github/workflows/foundation.yml` | `tests/contracts/descriptor.test.mjs` | `.` | ci |
| `root-test-ui` | `package.json` | `tests/ui/*.test.mjs` | `.` | full |
| `desktop-test` | `apps/desktop/package.json` | `tests/*.test.mjs` | `apps/desktop` | full |

层级由**读 `xtask/src/lib.rs`** 定：`engineering_checks` 是 `portable` 与 `full` 的公共前缀
⇒ 前三条 `portable`；`frontend::check` 只在 `full` ⇒ 两个 glob `full`；
`cargo xtask test --suite contracts` **不在** `check` profile 里（是独立 CI 步骤）⇒ `ci`。
在册测试文件 = **9 个**，全仓**唯一**扩展名 `.test.mjs`。

## 三、裁定

见 [ADR 0062](../adr/0062-test-reachability-closure.md)。要点：

1. `TEST_REGISTRATION_SITES`（七个站点，逐字写死）作单一来源；
   **覆盖集由 marker 原文解析得出**，MUST NOT 另行声明（否则「表 == 它自己」恒真）。
2. 用**已跟踪路径集**（`.git/index`，W75 的纯 JS 设施）而非 `readdir`：覆盖集按**仓库成员**算，
   未跟踪文件不参与 ⇒ 探针**必然**触发闭世界（本轮的证据正由此得到）。
3. **假契约**（marker MUST 逐字存在于 host）＋ **非空洞**（站点 MUST 至少覆盖一个在册文件）。
   「非空洞」**不是**「站点数 > 0」——后者对一个空 glob 是瞎的。
4. **闭世界 MUST 在「扩展名层」也成立**：`looksLikeTest`（宽）与扩展名 ∈ `TEST_EXTENSIONS`（窄）
   分开判。`foo.test.ts` 连「未被覆盖」都不会被报出——**清单的窄，正是它闭合的原因**。
5. **台账不是豁免**：`LOCALLY_UNREACHABLE` 条目 MUST 带理由、MUST 是在册测试文件、
   MUST **确实未被覆盖**（自清理，已可达即红）。
6. **「覆盖集非空」与「两集不相交」成对**：台账为空时不相交退化成恒真 ⇒ 必须有 `covered.size > 0`。
7. **入参显式注入，缺任一项即报错。**

## 四、实现

`scripts/contracts/test-reachability.mjs` 导出：
`TEST_EXTENSIONS`／`TEST_TIERS`／`TEST_REGISTRATION_SITES`／`LOCALLY_UNREACHABLE`（当前为空）／
`isTestFile`／`markerMatcher`／`resolveSite`／`siteShapeErrors`／`extensionErrors`／`reachabilityErrors`。

## 五、非空洞注入（F0 基线 + F1–F9 数据级 + T0–T5 表级）

数据级注入**在合成世界**上做（两个站点、两份 host 正文、三个在册文件），
因此「我改的确实是我以为的那一处」是**构造出来的**；表级注入用**临时模块副本**（该文件无 import）。

```
FIRE  F0  基线（合成世界）                        → 无错误
FIRE  F1  在册却不被覆盖的文件                      → 具名 reported
FIRE  F2  marker 不在 host 里（假契约）             → 具名 reported
FIRE  F3  glob 缩窄                                 → 恰好丢掉的文件落进闭世界，仍匹配的不报
FIRE  F4  marker 指向不存在的文件（空站点）          → 具名 reported
FIRE  F5  台账条目已可达（过期豁免）                 → 具名 reported
FIRE  F6  typed.test.ts（扩展名盲区）               → 具名 reported
FIRE  F7  真未覆盖的台账条目 + 非空理由（反向）       → 不红；无理由时红
FIRE  F8  站点顺序打乱（反向）                       → 不红；入参缺失 ⇒ 具名报错
FIRE  F9  站点表清空                                → 覆盖集为空 reported

FIRE  T0  复制模块 + 真实仓库                        → 无错误
FIRE  T1  从真实表删 root-test-ui                    → 恰好 4 个 ui 文件红（表剩 6 站点）
FIRE  T2  放宽 TEST_EXTENSIONS 到含 .test.ts         → widened=0 / narrowed=1（窄清单才是闭合的原因）
FIRE  T3  清空真实站点表                             → 10 条错误，含空覆盖
FIRE  T4  还原（逐字副本）                           → 无错误
FIRE  T5  源文件未被改动                             → 10498 字节不变

=== 开火/合计: 15 / 15 ===
```

> **F7 自身的一次构造错误（如实登记）**：第一版 F7 把文件**移出在册集合**来制造「未覆盖」，
> 结果判据报 `draft … which is not tracked` —— 因为台账条目 MUST 描述**仓库内容**。
> 这是**我的构造错了，不是判据错了**；改成由**站点侧**（缩窄 glob）制造未覆盖后即通过。
> 记这一笔：**注入用例也是断言，它的前提同样要被核实。**

## 六、验证

### 6.1 定向测试

```
node --test tests/registry.test.mjs            →  47/47 pass   （34 → 47）
node .w84_inject.mjs                           →  F0–F9 + T0–T5 共 15 项，全 FIRE
```

### 6.2 门控

```
cargo xtask check --profile full   →  EXIT=0
```

实测 **91 个测试目标（73 `Running` + 18 `Doc-tests`）、860 passed / 0 failed**，
日志内 `error[` / `LNK1104` / `out of memory` 命中 **0** 次。
口径以 `grep -c '^test result:' <门控日志>` 定准（**每个目标恰好一行，没有自由度**）；
`^     Running ` **74** 行 = **1 包装行**（cargo 自己打印的
`Running \`E:/nai-build-target\debug\nai-xtask.exe check --profile full\``，**不是测试目标**）+ 73 真实目标。
本轮**零 Rust 改动**，`passed` 与 W71–W83 逐项相同 ⇒「数字不变」正是「未误改别处」的旁证。
日志里可见本轮用例在门控内执行：`ok 36 - a test file nobody registers is reported by name`、
`ok 43 - an empty coverage set is reported, so "nothing covers anything" cannot pass`、
`ok 47 - the real repository reaches every tracked test file`
（Node 用例**不**计入 91/860，那组数只来自 `cargo test`）。

### 6.3 两条 validate

```
openspec validate --all --strict              →  43 passed / 0 failed
openspec validate --all --strict --archived   →  82 passed / 0 failed   （81 → 82）
```

另对**活动变更**单独跑 `openspec validate w84-test-reachability-closure --strict` ⇒ `valid`。
⚠️ **订正**：初稿此处曾照抄上一轮写「CLI 对 Why 段给了一条非阻塞警告」——**本轮没有这条警告**。
核实方式：① 归档命令只打印 `Task status: ✓ Complete` 与规格变更计数；② `openspec validate
<归档变更> --strict` 输出里**无任何 warn/⚠ 行**；③ 实测本轮 `proposal.md` 的 `## Why` 段
**521 字符**（阈值 1000）。**照抄上一轮的性状是不实陈述**，故就地订正为「无警告」。

## 七、CI 复核

**run `38339165763`（`head_sha = 3448eb51e2…`，与本轮推送 SHA 一致）= `success`**（run_number **98**，双平台绿）。

观测方式沿用既有纪律：**单次低频查询**（**不轮询**），每次带**唯一查询参数**破坏代理缓存、
`Cache-Control: no-cache`，并**回读 `head_sha` 与本次推送比对**；`curl` 带 `--tlsv1.2 --tls-max 1.2`
（schannel 与 TLS 1.3 握手失败，W77 实测）。本轮共 3 次查询：`in_progress`（推送后）
→ `in_progress`（等待 7 分钟后）→ **`success`**（`/jobs` 端点顺带确认）。

**jobs 明细**（`/actions/runs/38339165763/jobs`）：

```
foundation (ubuntu-24.04)  | completed success | steps=26 | failed: -
foundation (windows-latest)| completed success | steps=26 | failed: -
```

同一列表查询顺带证实上一轮 run `38333487130`（`head_sha = 0683ede8b1`，W83 回填）也是 `success`。

> **一处观测噪声（如实登记，非本轮引入）**：某一版单行 `node -e` 直接读 `curl` 的管道输出时
> `JSON.parse` 抛错并打印了 `RAW:`——**响应本身是合法 JSON**（44 004 字节），
> 与其后**存盘再解析**得到的结构完全一致（`total_count=97`）。判为**管道分块读取**的读侧问题，
> **不是端点异常**。⇒ 取证时**先落盘再解析**，别让读侧噪声污染判据。

**按观测轮终止约定**：本条回填提交会触发一个新的 run，**该 run 不再回填**——否则无限递推。

## 八、遗留

1. **判据只到「文本包含」**：站点 marker 存在于 host 文本中，不等于它会在一条**可达的执行路径**上
   被求值。若将来有站点把 marker 写进注释或死分支，本条**不会**红。需要「接线被真的走到」这一层时，
   应在**运行时**（而非静态文本）取证（ADR 0062 遗留 2）。
2. **`TEST_EXTENSIONS` 的窄是有意的**：将来引入第二种测试扩展名（如 `.test.ts`）必须先改这份清单。
   这是**想要的**摩擦，但它是一处需要记住的改动点——与 `CAPABILITY_PACK_IDS` 仍手写同类（ADR 0062 遗留 3）。
3. **§56.2 归档边界仍未写死**（W78 §八第 8 / W79 §八第 6 / W80 §八第 4 / W81 §八第 5 / W82 §八第 5 /
   W83 §八第 5 未变）：需一次**升基线轮次**把它写进基线正文。
4. **`CAPABILITY_PACK_IDS` 仍是手写数组**（ADR 0056 遗留 4 已给理由；本轮新增的
   `TEST_REGISTRATION_SITES` 与它**同类**——都是需要维护的手写声明，ADR 0062 决策 4 接受这份摩擦）。
5. harness 轨道 **H18 与 B07** 仍 `pending`；W76 台账的 5 条 `decidable_in_repo`
   （`PD-09/22/24/31/34`）仍只登记了推荐——**推荐的落地仍未发生**。
6. `cargo xtask schema --check` 本机仍跑不了（宿主对 node 派生任何子进程的 `EBUSY`）。
   本轮未改任何 schema 与生成器输入 ⇒ 零漂移由结构性理由 + CI 独立步骤兜底，**未写成「已通过」**。
7. **本轮新增了一个「需要维护的手写声明」**（`TEST_REGISTRATION_SITES`）。它是**有意的**：
   它使覆盖集非恒真。若将来站点增多到难以维护，可考虑由 `xtask` 侧**生成**这份表——
   但那会重新引入「生成物与事实漂移」的风险，需要另一次裁定。

## 九、方法论教训

1. **静默不跑比跑错更坏**：一个含 `assert.fail` 的文件被完整门控忽略、且零信号，
   比「跑了但断言写错」严重——后者至少留下一条红，前者让整体结论不可信。
2. **「列举」的守卫，盲区是列举的补集**——W80–W84 是同一个形状的五次应用
   （台账字段 → 台账字段集 → 台账信封 → 引用触发条件 → 门控接线）。
   下次看到「手写一份清单」，先问：**这份清单的补集由谁负责？**
3. **闭世界要在每个层级上成立**（W82 起）：文件之上还有**扩展名**一层。
   `foo.test.ts` 的盲区不在文件层——它连「未被覆盖」都不会被报出。
4. **「非空」是互斥类断言的必要一半**（W80 起，本轮又一次）：台账为空时
   「覆盖 ∩ 台账 == ∅」是**恒真**的，必须有「覆盖集非空」与它成对。
5. **注入用例本身也是断言，前提同样要核实**（本轮 F7 的教训）。
6. **排除干扰时要看「目标对象的痕迹」而非「红绿」**：探针那次红，
   真正的判据是**探针用例名出现 0 次**，而不是「重跑变绿了」。
