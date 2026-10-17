# Proposal: W135 — 「未跟踪」不等于「未登记」：把提交集的另一半补成一条判据

## Why

W134 的收口观测抓到一个**本机永远看不见**的缺陷：新增的 `tests/contracts/step-overlap.test.mjs`
没有登记进 `TEST_REGISTRATION_SITES`，而门控仍然**全绿**——因为那条闭世界的**输入是 `.git/index`**，
门控跑的时候这个文件还没 `git add`。推送之后 CI 一 checkout 就两平台同时红
（run `39016233527`，170 秒早退），annotation 逐字指向
`the reachability closed world must hold for the real repository`。

W134 修的是**实例**（把漏登记的文件补上），不是**类**。类仍然开着：

```
提交集 = 索引集（已跟踪）          ∪  补集（未跟踪且未被忽略）
         ↑ reachabilityErrors 判它      ↑ 没有任何判据判它
```

`resolveSite` 刻意不用 `readdir`（W74/W75：未跟踪 ⇒ 尚未承诺为仓库内容 ⇒ 不能充当覆盖证据），
那条理由对**覆盖**成立、对**提交**不成立：`git add -A` 会把补集里的文件收进提交。
于是「一个未登记的测试文件」在提交前对闭世界**结构性不可见**，提交后立刻可见
⇒ 本机绿、CI 红，而且**红在一条与该文件无关的断言上**，定位成本极高。

## What Changes

1. **新增补集读取器** `readUntrackedPaths`（`scripts/contracts/git-index.mjs`）：
   跑一次 `git ls-files --others --exclude-standard -z`。这是该模块**唯一派生进程**的地方，
   与它开头的立场（W74：子进程能力是宿主瞬态）**有意冲突**，理由与代价写在原地：
   忽略语义（`.gitignore` 分层、`info/exclude`、全局 excludes）只能问 git，自己实现会造出**更大的**
   盲区；代价是宿主不能派生进程时这里会失败 ⇒ 调用方 MUST 把它当**具名失败**，MUST NOT 当空集。
2. **新增补集判据** `pendingRegistrationErrors`（`scripts/contracts/test-reachability.mjs`）：
   「未跟踪且未被忽略」的 test-like 文件 MUST 已被某个登记站点的 marker 覆盖；未覆盖者具名报出，
   并把**怎么做**（登记站点 + 门控行 + 同一次提交里 `git add`）写进错误文本。
3. **补集必须是补集**：`present ∩ tracked ≠ ∅` 具名失败——否则调用方把整个提交集塞进来，
   两条判据会重复判同一批文件（「同一件事 MUST NOT 两处定义」的另一面）。
4. **非空洞两条前提**：`scanned > 0`（输入非空）与「清扫谓词不是惰性的」（合成输入必须被扫出来）。
   第二条是**注入 I3 现场发现的缺口**：把 `candidates.push` 关掉之后，真实仓库用例与 F0 **同时全绿**，
   只有单元用例会红 ⇒ 前提必须钉在真实仓库用例里。
5. **接线**：`xtask/src/lib.rs` 的 `engineering_checks` 加一行；`tests/registry.test.mjs` 新增一条真实仓库用例；
   站点表加 `xtask-pending-registration`，逐字清单 16 → 17。
6. **刻意不做**：
   - **不改 `resolveSite` 的输入**。把覆盖集从「已跟踪」换成「工作区」，会推翻 W74/W75 记录的
     理由，并让**未跟踪的伪造消费者**重新进入覆盖集（那正是 W74 F2 要修的东西）。补集判据是
     **正交**的加法，不是替换。
   - **不强制 `git add` 才能跑门控**：判据问的是「登记了没有」，不是「进索引了没有」——
     登记与提交是两件事，前者是本轮的可判对象。

## 具名登记（本轮**发现但不修**）

1. **补集读取依赖 `git` 可执行**。它不在 `.git/index` 的纯文件读取范围内，是**新的**失败面；
   本轮把失败做成**具名**（`reason: 'spawn'`），但没有第二读取路径（不实现 gitignore 是刻意的）。
2. **`target-check/` 仍在「未跟踪且未忽略」集合里**（本机 24,796 条、其中 0 条 test-like）。
   它对判据**当前**无害，但这是一份「会随缓存变化」的输入；把它加进 `.gitignore` 属另一轮
   （且删除受 safe-delete 批量阈值限制，memory 已记）。
3. **`registry.test.mjs` 的 F0 与新用例各自读一次 git**（两次进程派生，约 0.4 秒/次）。
   合并成一次读取会让两条判据共享一个输入 ⇒ 一处读坏两条同时变瞎，本轮选择**分开**。
