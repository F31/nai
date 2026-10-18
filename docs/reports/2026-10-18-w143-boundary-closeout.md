# W143 报告：三条「没有判定面」的尾巴（N4/N5/N6）

- 日期：2026-10-18
- 起点：W141 报告 §八 的 **N4**（`status=1`/`status=6` 缺判别性断言）、**N5**（`target-check/`
  未进 `.gitignore`）、**N6**（`openspec archive` 的 Why 段超长提示该怎么处置）。
- 交付：`status_domain.rs` +1 用例（两条字面断言）、`.gitignore` +1 条、ADR 0117、
  OpenSpec `w143-boundary-closeout`、三处过期表述的分流订正。

## 一、勘定与现测

四组数字全部本轮现测，不是从上轮推断。

| 项 | 现测值 | 怎么量的 |
|---|---|---|
| 补集条数（改动前） | **24,797**，其中 `target-check/` **24,796** | `git ls-files --others --exclude-standard \| wc -l` |
| 补集条数（改动后） | **0** | 同上 |
| Why 段超限（归档存量） | 139 目录 / **137** 可解析 / **2** 解析失败 / **32** 超 1000 / 最长 **2611** | upstream `ChangeParser.parseChangeWithDeltas` 逐条量 `change.why.length` |
| utterance status 裸数字断言 | `12` ×2（`tests.rs:485`、`migration.rs:555`）、`14` ×1（`approval_workflow.rs:202`）、**`1` 与 `6` 零条** | `grep -rn "assert_eq!(status"` 逐条看是不是这一列的数值 |

三条口径订正（都记在这里，避免下轮再算一遍）：

1. **Why 段长度只有一个口径**：`change.why.length`。memory 里记的「W141 2171 / W140 1953」
   既不是段长（1228 / 1171），也不等于整份 proposal 的长度（2951 / 2833），是第三种口径，作废。
2. **2 份解析失败是既有事实，不是本轮引入**：`2026-10-09-engine-assembly` 缺 What Changes、
   `2026-10-10-w31-golden-set-payload` 缺 Why。它们归档时 `validateChange` 只打警告（见 §四），
   所以一直没被修 —— 这是「非阻塞提示」的既有代价，登记在 §八。
3. **`validateChange()` 不是「无调用方」**。此前记录写的是「唯一消费者 `validator.js:77`，
   而 `commands/*.js` 都不调它」；本轮 grep 到 **`archive.js:1153` 真的在调**，只是包在
   `if (!json)` 里、且上游注释写明 `informative only (do not block archive)`。
   结论方向没变（它不阻塞），但坐标错了 ⇒ ADR 0117 用的是订正后的坐标。

## 二、N4：写入面四个取值的字面覆盖

**改动**：`crates/nai-storage/src/tests/status_domain.rs` 新增
`record_candidate_stores_one_and_approve_writes_six`。两条断言都走**生产路径**
（`record_candidate` 的 `INSERT`、`Store::approve` 的 `UPDATE`），都从**存储读回**，
都不引用常量。

**为什么这里允许裸字面量**（与文件头第 1 条纪律不冲突）：它钉的不是「常量写成了几」，
而是「**落盘的数据**是几」。`utterances.status` 是数据契约 —— `0001-foundation.sql` 的
`DEFAULT 1`、`0012` 的取值域 CHECK、既有库里已经躺着的历史行。常量写错会被编译期与
`utterance_status` 自己的用例发现；**常量改了而数据没跟着迁移，只有这里会红**。

**顺手去重**：`attempt_approval` 与 `cascade_outcome` 各自复写了一遍「起会话 → 发事实 →
登记候选」，加上新用例就是第三份。收成 `seed_candidate`（样本前置）、`set_status`（夹具直写）、
`stored_status`（从存储读回）三个函数，三条判据共用 —— 同一段前置不该在文件里躺三份。

**注入（两次，各自只令该用例变红）**：

| 注入 | 结果 | 失败文本 |
|---|---|---|
| `STATUS_PENDING: 1 → 2` | **只有**新用例红（6 passed / 1 failed） | `left: 2 / right: 1`，落点在第一条断言 |
| `STATUS_APPROVED: 6 → 5` | **只有**新用例红（6 passed / 1 failed） | `left: 5 / right: 6`，落点在第二条断言 |

两次注入后均逐字回滚：`git diff --stat` 只剩 `status_domain.rs`（`utterance_status.rs` 无 diff），
`cargo test -p nai-storage --lib` = **95 passed / 0 failed**。

⇒ 与既有三处合起来，写入面四个取值（`1`/`6`/`12`/`14`）各有字面断言；
且两条断言具**判别性**，不是「常量比常量」。

## 三、N5：构建缓存进忽略集

**改动**：`.gitignore` 加 `/target-check/`，带三行理由注释（同类于 `/target/`、后果、以及
「补集大小从来不是判定对象」的指针）。

**现测**：补集 24,797 → **0**；`git status` 不再出现 `?? target-check/`。

**具名代价**：那 24,796 条路径随之退出凭据扫描域（`xtask/src/secrets.rs` 用的就是同一条
`--cached --others --exclude-standard`）。接受它的理由不是「反正无害」，是三条：
①CI 上从来不存在这个目录 ⇒ 变动只让本机与 CI 的扫描域**一致**；②它与 `/target/` 同类且
`/target/` 从来在忽略集里；③需要被扫的产物，正确做法是**进仓库**（`/.nai-diagnostics/`
那段注释就是先例），不是留着不忽略。

**三处过期表述（分流处置，不搞一刀切）**：

| 载体 | 处置 | 为什么 |
|---|---|---|
| `tests/contracts/pending-registration.test.mjs` 注释 | 就地改 | 活判据 |
| `docs/development-plan.md` 两处 | 就地改 | 活文档 |
| `docs/reports/2026-10-17-w135-*.md` | 追加勘误段 | 已发布的历史记录 |
| `openspec/changes/archive/2026-10-17-w135-*/` | **不改** | 冻结快照；其「仍在补集里」是对当时状态的登记 |

订正时另抓到一处**笔误**（不是过期，是本来就不对）：那段注释说「必须与 `scanned > 0` 成对出现」，
而 `scanned > 0` 在 W135 同一轮就因为「输入是工作区状态」被**删掉**了 —— 用例里从来没有这条断言，
配对的另一半实际是「真实补集里没有待登记文件」。已在注释里改对，并在 W135 报告的勘误段里记明。

**验证**：`node --test tests/contracts/*.test.mjs tests/registry.test.mjs tests/platform/*.test.mjs`
= **293 passed / 0 failed**；`cargo test -p nai-xtask --lib secrets` = **16 passed / 0 failed**，
其中 `the_workspace_itself_is_clean` 与 `the_scan_domain_is_what_git_would_publish_and_excludes_ignored_state`
都通过 —— 后者正是这条改动碰到的那条判据，它没被放宽，判定口径仍是「等于 git 会发布的东西」。

## 四、N6：Why 段上限的裁定

**事实**（探针 + 源码 + 存量，三路互证）：

1. **`openspec validate` 不读 `proposal.md`**：造一份 Why 段 1414 字符的探针变更，
   `openspec validate --all --strict --no-interactive` 仍 `44 passed / 0 failed`，
   只多一行 `✓ change/zz-w143-probe`（探针已删）。
2. **`validateChange()` 有一个调用方，且刻意不阻塞**：`archive.js:1153`，在 `if (!json)` 内；
   注释原文 `// Proposal validation is informative only (do not block archive).`，
   打印头 `Proposal warnings in proposal.md (non-blocking):`。
   `commands/validate.js` 与 `commands/change.js` 只调 `validateChangeDeltaSpecs()`。
3. **阈值存在但只到提示为止**：`constants.js:8` 的 `MAX_WHY_SECTION_LENGTH = 1000`，
   `change.schema.js` 用 `.max(...)` 引用它，提示文案即 `CHANGE_WHY_TOO_LONG`。
4. **存量**：137 可解析 / 32 超限 / 最长 2611。近三轮 W140 = 1171、W141 = 1228、W142 = 601。

**裁定：不设门控**。理由按分量：

1. **执行点在依赖里，且它自己选择不阻塞**。改 `node_modules`（vendor）不做；自造一条新判据面
   是独立轮次，不是收口。
2. **它守的是阅读性，不是行为**。仓库里其余判据都能回答「不守会坏什么」；Why 段超长的后果是
   读者费劲 ⇒ **给不出「只有它错时才红」的样本**，判据无法证明自己必要。
3. **存量证明它从未被当作硬约束**：32/137。任何回溯式判定会立刻判红已发布的历史，
   修法只能是改写历史。

**代价（不付这个，裁定就成了「假装守住了」）**：四条事实进 ADR 0117 与本报告；
本轮变更自己把 Why 段控制在 **638** 字符（实测）—— 这是**样本**，MUST NOT 写成「已守住」。

## 五、诚实边界

- **不设门控 = 不设门控**。Why 段上限至今**没有人检查**。下次看到那行提示，它是提示，
  不是结论；也 MUST NOT 被引用成「已通过」。
- **凭据扫描域真的变小了**（少 24,796 条路径）。理由写清了，但这是一次**判据输入面的收窄**，
  不是零代价：若哪天有人在 `target-check/` 里写了凭据，扫描器不会报。缓解是「它不进仓库、
  凭据落在产物里成因在配置、配置在仓库内仍被扫」，不是「不可能发生」。
- **2 份归档 proposal 解析失败**（缺 Why / 缺 What Changes）至今没人修。它们不会被任何判据
  检查（见 §四），所以会一直留着；登记在 §八。
- **N4 只补了两个取值**。`13` / `15` 仍是「零写入者的幽灵值」：它们有分类、无写入者，
  没有字面断言可补 —— 没有生产路径会把它们写进存储。这不是缺口，是「写入面只有四个取值」
  的另一种说法。
- 本轮**没有**接手 W138 §30.1 第 4 条（12 条路由铸 `job_id` 而无状态来源）。

## 六、门控

_（回填）_

## 七、CI 观测

_（回填）_

## 八、具名登记（本轮发现但不做）

1. **2 份归档 proposal 结构性缺段**（`2026-10-09-engine-assembly` 缺 What Changes、
   `2026-10-10-w31-golden-set-payload` 缺 Why）。修它们等于改历史记录；不修则任何未来的
   「全量 proposal 可解析」判据都必须先给它们开例外。两种都要先裁定，本轮只登记。
2. **`13` / `15` 两个幽灵值**：有分类（`Unusable`）、无写入者。要么找到它们本该由谁写，
   要么从域里去掉 —— 但那要迁移，属独立一轮。
3. **凭据扫描域与「产物」的边界**：`/.nai-diagnostics/` 是「进仓库以便被扫」的先例，
   `target-check/` 是「不进仓库所以不被扫」的一侧。那条界线目前只靠注释表达，没有判据。
4. **`scanned > 0` 那条断言的缺位**：它被删掉是对的（输入是工作区状态），但「补集读取器
   真的读到了东西」这个前提，如今只由 `ok: false` 的失败路径与合成样本守着 ——
   真实仓库侧的「非空」从此不可断言。这是 W135 那次修复的固有代价，本轮把它写清楚。
5. **W138 §30.1 第 4 条**（12 条路由铸 `job_id` 而无状态来源）仍未接手。
