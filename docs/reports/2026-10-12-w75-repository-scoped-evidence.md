# W75 报告：仓库受限的消费证据（判据的输入必须等于它断言的对象）

- 日期：2026-10-12（W75）
- 相关：`docs/adr/0053-repository-scoped-evidence.md`、ADR 0052（其 §八遗留 1 提出的方向由本轮实现、
  其「必须派生 git 子进程」的陈述由本轮订正）、ADR 0051 决策 3
- 变更：`openspec/changes/2026-10-12-w75-repository-scoped-evidence/`（归档后移至 `archive/`）

## 一、判断与范围

出发点写的是 **W74 §八遗留第 1 条**：

> 未跟踪文件若落进「非 dot、非具名排除」的目录仍可能造出消费者……彻底消除需要「仓库文件集」
> 这个输入，而**取它需要派生 `git` 子进程（宿主禁止）**。

**核实把这条陈述拆成了两半**：

- 「**node 派生 `git` 子进程会 `EBUSY`**」——**成立**（W74 实测 `git` 裸名与绝对路径、
  `process.execPath`、`cmd.exe`、`sh` 一律 `EBUSY`）。这条**不动**。
- 「**因此取仓库文件集不可行**」——**不成立**。`.git/index` 是 Git 的 DIRC 格式，**纯 JS 可读**。

⇒ F2（W74 自己认定最严重的缺陷）**有根除路径**，不必停在「结构性收敛 + 登记残余风险」。
另牵出一条**新缺陷 F6**（扫描集的路径前缀格式不统一），它是本轮判据的前置条件。

**范围**：只动「消费守卫本身 + 它的输入面（扫描集与新增的仓库文件集读取器）」＋订正既有不实陈述。
**不改基线正文、不改任何 schema 文件、不改任何 `acceptance`、不改 Rust、不新增 xtask 步骤**
（解析器**不是**测试文件 ⇒ `engineering_checks` 零改动）。

## 二、核实（先读后判；探针落在 `dist/`，用完删除）

### 2.1 `.git/index` 可以纯 JS 解析（证伪 W74 的「不可行」）

```
magic=DIRC  version=2  entries=922  →  完整解析 922/922 条路径
```

DIRC 的头部是 12 字节（magic + 版本 + 条目数），条目是 62 字节固定头 + NUL 结尾的名字
（v2/v3 补齐到 8 字节倍数；v4 的名字是「公共前缀长度 varint + 后缀」且不补齐）。
**只需读一个文件，无需任何子进程。**

### 2.2 工作区扫描集与索引的关系（同期实测）

```
工作区扫描集（W74 裁定：跳 dot 段 + 4 条具名排除）  = 282
.git/index 全部路径                                  = 922
.git/index 里代码扩展名                              = 283
【A】工作区扫描到、但不在索引（即未跟踪）            = 0
【B】在索引（代码）但不在扫描集                      = 1   ← .github/workflows/foundation.yml
【C】工作区 ∩ 索引                                   = 282 = 扫描集
```

【A】= 0 说明**当前仓库干净**；【B】= 1 恰是 dot 目录里的工作流（W74 决策 1 的有意排除）✓。

### 2.3 F6：扫描集的路径格式不统一

`codeFiles()` 用 `dir.replace(/^\.\//, '')` 构造路径，而**初始调用 `dir === '.'` 时该替换不生效**
（`'.'` 不匹配 `^\.\/`）⇒ **根目录下的文件路径带 `./` 前缀**，子目录下的不带。
当前**无可观测影响**（`SELF` 恰在子目录、现有断言比较的都是子目录路径），
但**任何「与仓库路径集比较」的判据都会因此假阴性**——而本轮正是要做这个比较。
⇒ 它是本轮判据的**前置条件**，不是无害的历史遗留。

### 2.4 W74 之后 F2 的**实际**可利用性（一组三条实测）

构造攻击：真孤儿 `schemas/config/w75-orphan.schema.json` ＋ 未跟踪的 `.tmp-fake-consumer.rs`
（内含 `include_str!` 指向它——**不是**路径字面量，因为 W74 已移除那条来源）。

| # | 条件 | 实测 |
|---|---|---|
| V1a | 原样 | **两条断言同时开火**：`exclusiveTo(RUST)` 的**具名钉死**（孤儿的加入让 Rust 独有集合多一员）＋ 本轮的 `repositoryErrors` |
| V1c | 形状断言**放宽**为 `includes`（模拟「有人为了让自己的新消费者通过而顺手改松它」），保留 `repositoryErrors` | **`repositoryErrors` 独立兜住** |
| V1b | 两条都停用 | **17/17 全绿** ⇒ 洗绿成立 |

**这组实测比「F2 还在不在」更重要**：W74 之后 F2 难以洗绿，但挡住它的是一条**形状**断言，
**不是**关于**来源**的断言。那是**副作用，不是设计**——任何人**合法地**让一份新 schema 只被 Rust
消费，都会让那条钉死变红，而最顺手的处置就是**把它改松**（V1c 验的正是这个）。
⇒ **「碰巧挡住」不能算挡住。**

## 三、裁定（ADR 0053 的四条决策）

1. **仓库文件集 = `.git/index` 的路径集**：新增纯 JS 解析器（DIRC v2/v3/v4；`.git` 为文件时读
   `gitdir:` 指针），返回 `{ok, version, paths}` 或 `{ok:false, reason, detail}`。
2. **消费证据的来源 MUST ⊆ 已跟踪路径集**（`repositoryErrors`，越界时指名）。
   **为什么是「已跟踪」而不是「git 会发布的内容」**：`git ls-files --cached --others --exclude-standard`
   **含未跟踪文件** ⇒ 那个伪造消费者也在集合里，**修不了 F2**；守卫跑在**提交前**，
   「未跟踪」正是「尚未承诺为仓库内容」。
   **为什么约束「证据来源」而不是「扫描集」**：后者会让任何尚未 `git add` 的代码文件把门控**假红**
   （闭环在提交时才 `git add`），而攻击面恰在证据来源上 ⇒ 精确打击，零假阳性。
3. **扫描集路径是 POSIX 相对路径**（修 F6）＋「MUST NOT 有 `./` 前缀」断言。
4. **索引不可解析 MUST 失败**且**说得清是哪一类**（`missing`/`bad-magic`/`unsupported-version`/
   `truncated`/`corrupt`）——「判据的输入等于被断言的对象」是这条守卫的前提。

**否证过的备选**（详见 ADR）：`git ls-files --cached --others`（含未跟踪，修不了 F2）、
由 xtask 导出清单（顺序耦合 + 陈旧产物）、不做（缺口可关掉，继续登记不是唯一负责任选项）、
解析失败时降级为「通过并打印一行」（保护会在不知情时消失）。

## 四、实现

| 文件 | 改动 |
|---|---|
| `scripts/contracts/git-index.mjs` | **新增**：`parseIndex(buf)`（纯函数）、`readTrackedPaths(root)`、`describeFailure(failure)` |
| `tests/contracts/schema.test.mjs` | `codeFiles()` 路径改为 POSIX 相对路径；`consumptionOf` 返回 `{consumed, evidenceSources}`；新增 `repositoryErrors`；新增两组测试（解析器字节流、真实索引一致性） |

`parseIndex` 做成**接字节流的纯函数**，是为了让 v2/v3/v4 与四类失败**可构造**——
否则「它认得格式」这句话就没有证据。

## 五、非空洞注入（每组都必须被抓住，且逐项还原）

| # | 注入 | 预期 | 实测 |
|---|---|---|---|
| V1a | 真孤儿 + 未跟踪的 `.rs` 伪造消费者（`include_str!` 形态） | 报错并指名 | **被抓住**（两条断言同时开火） |
| V1c | 形状断言放宽、保留 `repositoryErrors` | `repositoryErrors` 独立兜住 | **被抓住**（新判据不是重复的） |
| V1b | 两条断言都停用 | 洗绿 | **17/17 全绿**（证明它是唯一防线） |
| V2a | 索引不可读（指向不存在的文件） | `missing` + 可诊断 | **被抓住**（`missing (.git\index-… is not readable (ENOENT)) — …`） |
| V2b | magic 不符 | `bad-magic` | **被抓住** |
| V3 | 扫描集带 `./` 前缀 | 路径格式断言变红 | **被抓住**（`the scan set must not carry a "./" prefix`） |

外加**解析器字节流单测**（构造输入）：v2（短名与长名混排，暴露补齐全错）、v3（扩展 flags）、
v4（前缀压缩 + 不补齐）、坏 magic、版本 5、两种截断、corrupt（名字首字节是控制字符）、空索引。

**还原核验**：每次注入都以编辑回退 + `git status` 确认；攻击文件用 `unlinkSync` 删除；
最终 `git status` 为「1 个已改文件 + 2 个新增（变更目录、解析器）」。

## 六、验证

- **消费守卫单测**：`node --test tests/contracts/schema.test.mjs` ⇒ **17 / 17 通过**（15 → 17）。
- **门控 full**：`cargo xtask check --profile full` ⇒ **EXIT=0**；测试目标 **91（73 `Running` + 18 `Doc-tests`）**、
  **858 passed / 0 failed**（与 W73/W74 逐项相同——本轮零 Rust 测试改动）。
  门控日志中可见新增的两项：`ok 16 - the tracked-path reader parses DIRC v2/v3/v4 …`、
  `ok 17 - the tracked-path reader agrees with the repository it is asked about`
  ⇒ **新断言确实在门控里执行**，不是只在本机单跑时成立。
- **两条 `openspec validate`**：`--all --strict` = **43**；`--all --strict --archived` = **73**（W74 为 43 / 72）。

## 七、CI 复核（匿名观测）

| run | head_sha | 结论 |
|---|---|---|
| [`38237572621`](https://github.com/F31/nai/actions/runs/38237572621) | `638de56`（与本轮推送 SHA 一致） | **ubuntu-24.04 `success`**（含第 10 步「Complete engineering foundation」＝ portable 步骤 + 前端门控）；**windows-latest `failure`**（同一第 10 步）⇒ **run 总状态 Failure** |

### 7.1 本轮要核对的那条假设：**已证实**

`.git/index` 在 CI 上可解析。判据是 **ubuntu job 的第 10 步 success**——那一步包含
`schema::node(… "tests/contracts/schema.test.mjs")`，即本轮新增的「证据来源 MUST ⊆ 已跟踪路径集」
与解析器断言**在 CI 上真的跑过且通过**。⇒ 决策 4 的「索引不可解析必须失败」在 CI 上**没有**被触发，
说明 `actions/checkout@v4`（无 `fetch-depth` / `index.version` 配置）产出的确实是可解析的 v2 索引。

### 7.2 windows 失败的根因（**如实登记：本轮 CI 红**）

失败点全部在 **`crates/nai-control-api/src/tests.rs:364`** 的**测试准备阶段**：

```
called `Result::unwrap()` on an `Err` value: StorageError { code: DeadlineExceeded, reason: "operation stopped", position: None }
```

四个 approval 相关测试因此 FAILED（`approval_busy_backend_and_full_ledger_fail_bounded`、
`approval_approve_retry_conflict_and_reject_are_scoped`、`approval_list_and_real_reject_workflow_over_loopback`、
`approval_invalid_schema_epoch_revision_hash_and_expired_ttl_do_not_execute`）。

**判定：windows runner 上的时序 flaky，不是本轮引入。** 四条判据：

1. **本轮零 `.rs` 改动**（`git show --name-only 638de56` 里没有任何 `.rs`）；
2. **ubuntu job success**（同一份代码、同一个第 10 步）；
3. **本机 `check --profile full` EXIT=0**（含这 4 个测试）；
4. 失败在**准备阶段**的 `set_permission(&op, …).unwrap()`，不是被测逻辑的断言失败 ⇒ 与环境速度相关
   （windows runner 一贯更慢：W74 的 windows 10m29s vs ubuntu 5m33s）。

**它顺带暴露一个真实的可判定缺陷（登记，未在本轮处理）**：`DeadlineExceeded` 被 `unwrap()`
变成 panic，而不是**具名的诊断**——这正是「宿主瞬态条件 MUST NOT 改变被测系统的结论」的一个反例
（W70 已对 keeper 演练做过同类修正：把主机限制改为主机限制的**可命名**诊断）。本轮范围是消费守卫，
不改 Rust ⇒ 留给后续轮次。

**后续**：本条回填提交会触发一个新的 run；按观测轮终止约定，**该 run 不再回填**。
若它绿，则上面的 flaky 判定被证实；若它红在**同一处**，则需要开一轮专门处理
「deadline → panic」的诊断化（而不是继续归因于环境）。


## 八、未做与遗留（如实登记）

1. **`.github` 仍被结构性排除**（沿用 W74 决策 1）：将来若工作流成为某 schema 的唯一消费者，
   会被报成孤儿（假阳性，安全方向），处置是加一条具名例外。
2. **依赖 `.git` 内部格式**：DIRC v2 是二十年默认；解析器自带 v2/v3/v4 与四类失败的字节流单测；
   版本不支持时明确失败（不猜）。**这是引入的唯一新依赖面。**
3. **「已跟踪」是索引不是 HEAD**：已 `git add` 但未提交的文件算「仓库内容」（可接受：`git add`
   是意图提交的显式声明）。
4. **`exclusiveTo` 的具名钉死仍脆弱**（§二 2.4）：它现在不再**承担** F2 的防线，但仍是
   「来源自负其责」的判据本身；将来**合法地**新增只被 Rust 消费的 schema 时，该断言会红并**要求
   更新它的成员清单**——这是有意的（要求人做一次判断），但**不该被当作「形状对就行」的信号**。
5. **语义类漂移仍只能靠人读**（沿用 ADR 0050/0051/0052 的登记）。
6. **`rag-golden/case-set` 的例外理由**仍需人维护。
7. **`§56.2` 自身两处张力、`§57.5`/`§48.7` 待决策项**：沿用 W71–W74 的登记，未动。
8. **`schema --check` 仍在本机跑不了**（宿主禁止 node 派生子进程）：取零漂移仍靠结构性理由 +
   CI 独立步骤；**不得把「跑不了」写成「已通过」**。
9. **windows runner 上 `nai-control-api` 的 deadline 超时被 `unwrap()` 变成 panic**（见 §7.2）。
   本轮判为**环境 flaky**（四条判据），但「宿主瞬态条件 MUST NOT 改变被测系统的结论」这条铁律
   在这里**没有落地**——超时应当是**具名诊断**（W70 对 keeper 演练做过同类修正），而不是 panic。
   **登记为后续轮次的候选**（需要改 Rust）。

## 九、方法论教训

1. **「不能」与「不能这样」是两回事。** W74 把「node 不能派生 `git`」正确地当成了事实，
   却把「因此不能取仓库文件集」当成了结论——**那个结论比它的前提强得多**。
   ⇒ **凡写下「因此不可行」，都要问：不可行的是那一步，还是整条路？**
2. **一条断言可能因为错误的原因通过。** W74 之后 F2 没被利用，不是因为判据在说「来源必须在仓库里」，
   而是因为一条**形状**断言（`exclusiveTo` 的钉死）**顺带**挡住了它。⇒ **判据必须说出它要的东西**；
   「没被利用」与「不可能被利用」之间的差距，在有人**合法地**触发那条形状断言时就会暴露。
3. **「碰巧挡住」不能算挡住。** V1c 是这个教训的可执行形式：把形状断言**改松**（一个完全合理的
   维护动作），保护就消失——除非有一条**语义正确**的判据独立成立。
4. **修 A 之前先量 B 的独有覆盖**（W74 教训 5 的姊妹）：本轮移进一条**新能力**（读索引）之前，
   先量了它**能不能真的代替**旧做法——`git ls-files --cached --others` 看起来是「现成的仓库文件集」，
   量了才发现它**含未跟踪文件**，在 F2 上等于零。
5. **订正一条归因要扫全仓，订正一条「不可能」也一样。** 「取仓库文件集不可行」这句话落在
   `AGENTS.md`（活文档 ⇒ 就地 1:1 订正）、`docs/adr/0052` 两处、W74 报告一处（历史记录 ⇒ 追加勘误段）。
   同时要**区分同形不同义**：`AGENTS.md` 与 `docs/development-plan.md` 里另有 `git ls-files` 的用例
   （`check --secrets` 的扫描域），那是在 **Rust** 里跑的，**不受 node 约束**——一并改掉就是**误伤**。
