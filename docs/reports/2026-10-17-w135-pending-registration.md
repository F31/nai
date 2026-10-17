# W135 — 「未跟踪」不等于「未登记」

> 起点：W134 收口时 CI 抓到的那条本机看不见的缺陷。W134 修的是**实例**，本轮补的是**类**。

## 一、勘定

### 1.1 起因不是某条验收条目

`tests/registry.yaml` 本轮**未改**（现测：`acceptance` **88** 条 = tested 12 / implemented 26 /
pending 42 / blocked 5 / not_applicable 3，与上轮逐项相同）。本轮处理的是 W134 收口观测里的一条事实：

- run `39016233527`（`6c574ec`）**两平台 `failure`**，170 秒早退在 gate 步；
- 匿名 annotation 逐字指向 `the reachability closed world must hold for the real repository`；
- 成因：新增的 `tests/contracts/step-overlap.test.mjs` 没登记，而那条闭世界的**输入是 `.git/index`**，
  门控跑的时候该文件还没 `git add`。

W134 的修法（把文件登记上）解决了**这一个**缺陷，没有解决**这一类**：下一个新测试文件会重演。

### 1.2 提交集被劈成了两半，只有一半有判据

```
提交集 = 索引集（已跟踪）                        ∪  补集（未跟踪 且 未被忽略）
         └─ reachabilityErrors 判它                  └─ 本轮之前：**没有任何判据**
```

`resolveSite` 刻意不用 `readdir`（W74/W75：未跟踪 ⇒ 尚未承诺为仓库内容 ⇒ 不能充当覆盖证据）。
那条理由对**覆盖**成立，对**提交**不成立：§5 的暂存命令是 `git add -A -- …`，它会把补集收进提交。
⇒ 一个未登记的测试文件在提交前对闭世界**结构性不可见**，提交后立刻可见。

### 1.3 现测三件事

| 事实 | 值 | 怎么测的 |
|---|---|---|
| 补集（未跟踪且未被忽略）规模 | **24,796** 条 | `git ls-files --others --exclude-standard -z` |
| 其中 test-like | **0** 条 | 同一份输出过 `looksLikeTest` 正则 |
| 补集的唯一来源 | `target-check/`（构建缓存，未被 `.gitignore` 覆盖） | 逐条看前缀 |
| 索引里的 `.test.mjs` | **18** 个 | `git ls-files \| grep -cE '\.test\.mjs$'` |
| 从 node 派生 `git ls-files` | 成功，448ms | 一次性实测 |

最后一行**不是**结论：这次能派生不代表下次能（W74 实测过 `EBUSY`）。
⇒ 设计上它必须是**具名失败**路径，而不是「不可用就跳过」。

### 1.4 为什么既有判据看不见

| 判据 | 它管什么 | 对「未跟踪的新测试文件」 |
|---|---|---|
| W84 闭世界（`reachabilityErrors`） | 已跟踪的测试文件谁覆盖它 | **沉默**（输入就是索引） |
| W95 步集 / W97 覆盖闭包 / W100 执行配置 / W101 目标绑定 / W134 覆盖关系 | CI 执行面 | 无关（这些判的是 workflow） |

五层都在，五层都对「**即将提交**的东西有没有人跑它」沉默。W134 记过一条方法论——
**判据集合的盲区 = 各判据的交集之外**；本轮是它的第二次适用，只是这次盲区的形状是「时间的」：
不是「没人判这个维度」，而是「判据比提交早了一步」。

### 1.5 一条被刻意保留的立场

`git-index.mjs` 开头明写「**不依赖子进程**」（W74：`git`、`node`、`cmd.exe`、`sh` 一律 `EBUSY`
——但那是一次宿主观测）。本轮**没有**推翻它，而是把例外写在原地：

- 覆盖集仍只按 `.git/index` 算（未跟踪的东西不能充当覆盖证据）；
- 新增的读取器**只读补集**，与覆盖判据互不重叠（由判据自己断言 `present ∩ tracked == ∅`）。

替换（把 `resolveSite` 的输入换成工作区）会让**未跟踪的伪造消费者**重新进入覆盖集——
那正是 W74 F2 要修的东西。正交的加法 ≠ 替换。

## 二、交付

1. **`readUntrackedPaths`**（`scripts/contracts/git-index.mjs`）：一次
   `git ls-files --others --exclude-standard -z`；失败返回 `{ok:false, reason:'spawn', detail}`，
   **MUST NOT** 退化成空集。
2. **`pendingTestFiles`**（`test-reachability.mjs`）：返回 `{scanned, candidates}`——
   「一个都没扫到」与「扫到了但都合格」是两件事，前者必须被**清点**成 `0`。
3. **`pendingRegistrationErrors`**（同上）：补集里每个 test-like 文件 MUST 被某个站点的
   `markerMatcher` 命中；未命中者具名报出，并把**修法**写进错误文本；扩展名不进 `TEST_EXTENSIONS`
   者按「扩展名缺口」具名；`present ∩ tracked ≠ ∅` 按「补集不是补集」具名。
4. **接线**：`xtask/src/lib.rs` 的 `engineering_checks` 加一行；
   `tests/registry.test.mjs` 新增真实仓库用例（56 → **57** 条）；
   站点表加 `xtask-pending-registration`（tier `portable`），逐字清单 16 → **17**。
5. **`tests/contracts/pending-registration.test.mjs`**：11 条，含两条**前提**（见 §3.4）。

## 三、判据

### 3.1 十一条

| # | 用例 | 钉什么 |
|---|---|---|
| 1 | the shipped repository has no unregistered pending test file… | 真实仓库 + 两条前提 |
| 2 | an unregistered pending test file is named, with the fix in the message | 具名 + 修法在文本里 |
| 3 | a pending test file that a site already declares is not named | **负方向**（不是铁锤） |
| 4 | a glob marker covers a pending test file as well | 两半共用同一个 matcher |
| 5 | …unknown extension is named as an extension gap | 扩展名闭世界 |
| 6 | missing inputs are named… | 缺入参 ⇒ 具名（两条都要） |
| 7 | a pending set that overlaps the index is named | 补集必须是补集 |
| 8 | an empty pending set is counted as zero, not skipped | 空集要清点 |
| 9 | an empty site table turns every pending test file red | 站点表清空不得变绿 |
| 10 | the untracked reader fails with a named reason… | 读取失败 ≠ 空集 |
| 11 | the shipped site table covers every test file the index holds | 站点数下限（16） |

### 3.2 注入验证（8 组）

| 组 | 注入 | 结果 |
|---|---|---|
| **I1** | **不打补丁**：工作区放一个未登记、未跟踪的 `tests/contracts/w135-probe.test.mjs` | 新判据**红**（1 条，逐字点名探针）；**旧闭世界（F0）依旧全绿** ⇒ 盲区被现场复现 |
| I2 | 判据的 `covered` 分支失效（`if (false && !covered)`） | 3 红（1 / 2 / 9） |
| I3 | 清扫谓词惰性（`if (false) candidates.push(...)`） | 首跑：**1 与 F0 同时绿**（缺口！）⇒ 补前提后 4 红（1 / 2 / 5 / 9） |
| I4 | 扩展名分支失效 | 2 红（1 / 5） |
| I5 | 重叠检查失效 | 2 红（1 / 7） |
| I6 | 读取失败退化成 `{ok:true, paths:∅}` | 2 红（1 / 10） |
| I7 | 缺入参检查失效 | 2 红（1 / 6） |
| I8 | **负方向**：把「已登记」判成「未登记」（`covered = false`） | 4 红（1 / 3 / 4 / 7）⇒ 负方向有判别力 |

**如实披露**：I2–I8 期间探针文件一直在盘上，所以每条里的「1 红」不能单独归因给该次注入
（它同时受探针影响）；**归因只取单元用例那几条**（2 / 5 / 6 / 7 / 9 / 10）。

### 3.3 现场证据：判据先抓住了它自己

本轮的测试文件诞生时**本身就是**一个「未跟踪、未登记的新测试文件」。写完判据、先不登记就运行：

```
test file tests/contracts/pending-registration.test.mjs is in the working tree but not in .git/index,
and no registration site covers it — the closed world only reads the index, so until it is registered
this file runs nowhere locally and the gate stays green while CI turns red on the very first checkout:
register it in TEST_REGISTRATION_SITES, add its gate line, and `git add` it in the same commit
```

登记之后同一条用例转绿。**这不是装饰**：它是「判据能走到那一层」的现场证据，
比任何合成注入都强（W128 的第二种成因在这里不成立——不是构造性事实，是能变的）。

### 3.4 两处自造缺陷（当场修掉，如实记录）

1. **把「清扫结果」当成了「未登记集合」**：首版的第 1 条写 `assert.deepEqual(candidates, [])`，
   而 `candidates` 是「待登记的 test-like 文件」（登记之后它**仍然非空**，因为文件还是未跟踪的）。
   正确写法是**独立复算**：每个候选都必须被某个站点命中。
2. **`I3` 暴露了一个真空洞**：把清扫谓词关掉之后，真实仓库用例与 F0 **同时全绿**——
   `scanned > 0` 只证明「输入非空」，不证明「清扫会把符合形状的路径收进来」。
   修法：在真实仓库用例里补一条**合成输入**的前提（`__sweep_probe__.test.mjs` 必须被扫出来），
   并在注释里写明它为什么必须与 `scanned > 0` 成对。

## 五、具名登记（本轮**发现但不修**）

1. **补集读取依赖 `git` 可执行**。这不是 `.git/index` 的纯文件读取能覆盖的失败面，是**新增**的；
   本轮把它做成**具名**（`reason:'spawn'`），但没有第二条读取路径——不实现 gitignore 是**刻意**的
   （自己实现会造出「以为被忽略、其实会被提交」的更大盲区）。
2. **`target-check/` 仍在补集里**（本机 24,796 条、0 条 test-like）。对判据当前无害，
   但这是一份**会随构建缓存变化**的输入；把它写进 `.gitignore` 属另一轮
   （删除受 safe-delete 批量阈值限制，memory 已记）。
3. **真实仓库用例与 F0 各读一次 git**（两次进程派生，约 0.4 秒/次）。合并成一次会让两条判据共享
   一个输入 ⇒ 一处读坏两条同时变瞎。本轮选择**分开**。
4. **判据不覆盖「已跟踪但已修改」的内容**：它判的是「有没有登记」，不判「内容改对了没有」。
   后者由各自套件的用例判——同一件事 MUST NOT 两处定义。
5. ⚠️ **站点 id 撞上了凭据扫描器的前缀规则**（本轮实测，见 §4.2）：`sk-` 后跟 ≥32 个字符会被判成
   `openai-style` 凭据。本仓库的站点 id 约定 `xtask-…` 天然带 `sk-` 前缀
   ⇒ **id 名字长度成了安全扫描的变量**。本轮把 id 改短避开，判据的表本身**未修**。

## 四、门控

### 4.1 五次，四次红，成因**三类**

| 次 | 结果 | 耗时 | 成因 |
|---|---|---|---|
| 第 1 次 | `EXIT=1` | 150s | `LNK1104`：链接 `target\debug\deps\nai_xtask-8cb62b445dfc3b8d.exe` 失败，`tasklist` 无残留进程 ⇒ 宿主瞬态 |
| 第 2 次 | `EXIT=1` | 179s | **真实缺陷（代码）**：`secrets::tests::the_workspace_itself_is_clean` 报 5 处 `openai-style` |
| 第 3 次 | **`EXIT=0`** | 143s | —（本轮首次全绿） |
| 第 4 次 | `EXIT=1` | 169s | **真实缺陷（文档）**：同一个 id 在 dev-plan / knowledge / 本报告里又写了 3 遍 ⇒ 3 处命中 |
| 第 5 次 | **`EXIT=0`** | 180s | —（收口） |

⚠️ 四次红**不是同一个原因**，所以 MUST NOT 记成「宿主一直在动」：第 1 次是链接期的宿主瞬态，
第 2/4 次是判据抓到了我自己的书写缺陷（而第 4 次是**改文档时把同一个坑又踩了一遍**——
修「代码里的 id」不等于修「这个字符串在仓库里的所有出现」）。

### 4.2 第 2 次：判据把我的命名法判成了凭据

5 处命中逐条是：

```
openspec/changes/w135-pending-registration/proposal.md:39
openspec/changes/w135-pending-registration/tasks.md:29
scripts/contracts/test-reachability.mjs:140
tests/registry.test.mjs:2156
tests/registry.test.mjs:2168
```

全部来自同一个新站点 id：`xtask-` + `engineering-pending-registration`。`xtask/src/secrets.rs` 的规则是
`prefix: "sk-", min_len: 32` —— `xtask-` 里天然含 `sk-`，而 `engineering-pending-registration` 恰好
**32 个字符**。⇒ 一个**测试站点 id** 被判成了 OpenAI 凭据。

**处置**：id 改短为 `xtask-pending-registration`（`sk-` 之后 20 个字符），5 处全改；
单跑该用例 `secrets::tests::the_workspace_itself_is_clean` **46.20s 通过**。
**判据本身没有改**（改的是我的命名）——理由见 §五.5：前缀规则的边界应当由命名避让，而不是由豁免削弱。

⚠️ **第 4 次门控红是同一个坑的第二次**：改完代码里的 id 之后，我在 dev-plan / knowledge / 本报告里
又把这个长 id **逐字**写了一遍（3 处：`docs/development-plan.md`、`docs/knowledge.md`、本报告 §4.2）
⇒ 同一批命中换个文件再来一次。修法是把文档里的写法改成**断开形式**
（`` `xtask-` + `engineering-pending-registration` ``）：`-` 之后的空格不是 `TOKEN_CHARS`，run 在 12 字符处就断了。
⇒ **教训**：修「一个字符串撞了扫描器」，落点是**这个字符串在仓库里的所有出现**，不是命中的那一处。

### 4.3 收口那次（第 5 次）

`cargo xtask check --profile full` **`EXIT=0`**，**180s**，`LNK1104` **0** 次：

- Rust **1085 / 0**（与 W134 逐项相同——本轮**零** Rust 测试变动，只加了一行 `node --test` 调用）；
- Node **348 / 0**，**15** 个套件（W134 是 336 / 0、14 套件 ⇒ **+12 / +1**：新判据 11 条 + registry 新增 1 条）；
- `Running` 83 行 / `Doc-tests` 18 行（与 W134 同形；`100 < 83+18` 是 cargo 既有形状，不作逐轮对比指标）；
- 错误类（`LNK1104` / `error[E…]` / `error:`）**0**；前端 7 步全通过。

（第 3 次也绿，143s，计数完全相同；第 5 次是 §六 修复之后的收口观测。）

**逐字节还原的边界（如实披露）**：I1–I8 期间三个文件的 `sha256sum -c` 还原核对中，
两个**源模块**（`test-reachability.mjs` / `git-index.mjs`）逐字节回到注入基线
（`c5a6600…` / `2f957cd…`）。此后 `test-reachability.mjs` 因 §4.2 的**站点 id 改短**又被合法改了一次
⇒ 它最终的字节**不等于**注入基线。这不是还原失败，是「基线之后还有一次真实改动」；
测试文件也同理（§3.4 的前提是在基线之后补的）。

## 六、勘误：一条**只有 CI 会红**的非空洞前提（已修）

**首推 `85ae008` → run `39021305527` 两平台 `failure`，161 秒早退。** 逐平台的失败步骤是同两步：
第 10 步 `Complete engineering foundation`、第 12 步（`if: failure()` 的诊断扫描，失败时必然跟着跑）。
匿名 annotation 在两平台上给出**同一条**断言：

```
error: 'the untracked scan returned nothing — an empty input makes the pending judge vacuous'
name: 'AssertionError'
```

**成因**：`assert.ok(scanned > 0)` 里那个 `scanned` 是**补集的大小**——它是**工作区状态**，不是判定对象。
本机扫到 **24,796** 条（未被忽略的 `target-check/`），CI 的干净 checkout 扫到 **0** 条。
⇒ 这条「非空洞前提」在 CI 上**恒假**：判据本身没问题，**前提的输入选错了**。
这与 W131 记的「本机门控与 CI 并行度不一致」是**同型**的机制性差异：本机绿、CI 红，且都不说明代码有缺陷。

**修法（两处用例 + 一处活规格）**：
1. 删掉 `scanned > 0`（`tests/contracts/pending-registration.test.mjs` 与 `tests/registry.test.mjs` 各一处），
   反空洞的落点改为**合成输入**前提（`__sweep_probe__.test.mjs` 必须被扫出来）——它在两种环境里都成立。
2. 活规格 `openspec/specs/engineering-foundation/spec.md` 的那条 Scenario 就地 1:1 改成两条 `THEN`，
   并写明「补集的大小 MUST NOT 被当成前提」。归档的变更包是**已发布的历史记录**，原文保留，勘误记在本节。

**复现实验（不靠 CI）**：把补集当空集喂进去 ——
`pendingRegistrationErrors({present: new Set(), tracked: <真实索引>})` ⇒ `[]`。
即修复之后，判据的结论**不再依赖**工作区里恰好有多少未跟踪文件。

⚠️ **这是连续第二轮「本机绿 / CI 红」**，两次的机制不同、教训相同：
W134 是「判据的**输入**少了新文件」（未跟踪 ⇒ 不在扫描集），W135 是「判据的**前提**用了工作区状态」。
两者都已经写进 `MEMORY.md`。
