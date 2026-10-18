# ADR 0117：两条「判据够不着」的边界 —— 构建缓存入忽略集，Why 段上限只当软提示

- 状态：已接受（W143，2026-10-18）
- 基线：`docs/直播数字人 + 智能体系统技术方案 V3.11.md`
- 相关：W141 报告第八节（N4/N5/N6）、W142 报告第八节（M4/M5）、
  `.gitignore`、`tests/contracts/pending-registration.test.mjs`、
  `scripts/contracts/git-index.mjs` 的 `readUntrackedPaths`、`xtask/src/secrets.rs`、
  ADR 0067（同一形状：把「没有判定面」这件事本身写清楚）

## 背景

W141 第八节把三件事留给后续轮次：**N4**（`status=1`/`status=6` 缺判别性断言）、
**N5**（`target-check/` 未进 `.gitignore`）、**N6**（`openspec archive` 的
「Why section should not exceed 1000 characters」该怎么处置）。W143 一次收口。

N4 是判据补缺，落在 `crates/nai-storage/src/tests/status_domain.rs`，不需要裁定。
N5 与 N6 的共同形态是**没有判定面**：一件是本机构建缓存站在仓库边界外，一件是依赖里的
一条非阻塞提示。两者的真问题都不是「改哪一行」，而是「**该不该被守，以及不守的代价说清楚了吗**」。

## 决定一：`target-check/` 进 `.gitignore`（N5）

**事实（现测）**：W112 起本机门控用 `CARGO_TARGET_DIR=target-check` 跑第二份构建缓存；
该目录 12G、未跟踪、未被忽略 ⇒ 一直躺在 `git ls-files --others --exclude-standard`
的补集里，24,797 条中的 24,796 条来自它。加入忽略后复测：补集 **0** 条，
`git status` 不再出现 `?? target-check/`。

**具名的代价（这一条 MUST NOT 被省略）**：那 24,796 条路径随之退出凭据扫描域 ——
`xtask/src/secrets.rs` 的扫描域就是同一条 `--cached --others --exclude-standard`。
接受它的三条理由，按分量排：

1. **CI 上从来不存在它**。干净 checkout 没有 `target-check/`，所以这 24,796 条从来只在本机
   被扫。变动之后本机与 CI 的扫描域**一致**，不是放弃了一条本机独有的保护。
2. **它与 `/target/` 同类**（构建产物），而 `/target/` 从第一天就在忽略集里。
   凭据落在构建产物里，成因在构建配置，而配置在仓库内、仍被扫。
3. **`/.nai-diagnostics/` 证明了正确方向**：需要被扫的产物，做法是**让它进仓库**
   （那里写着「刻意住在仓库内，好让扫描器有明确的扫描域」）。`target-check/` 不属于那类。

**否决的替代**：

- **删掉目录**：受 safe-delete 批量阈值限制（`SAFE_DELETE_BULK_CONFIRM_REQUIRED`，memory 已记），
  且下次门控会重建 ⇒ 治不了根。
- **写进 `.git/info/exclude`**：本机私有 ⇒ 每个新 clone 再踩一次；而「暂存事故」是仓库级问题。
- **维持现状 + 每次显式 `git add -- <path>`**：这与「用纪律替代机制」等价，且已两次复现
  （漏 `':!target-check'` ⇒ 残留 `.git/index.lock`）。忽略之后这条排除项不再是必需品，
  但 §5 的纪律**保留**——忽略不覆盖「已跟踪后被修改」的那一半。

## 决定二：Why 段 ≤1000 字符**不设门控**（N6）

### 事实（逐条可复核）

1. **`openspec validate` 根本不读 `proposal.md`**。探针变更 `zz-w143-probe` 的 Why 段
   1414 字符，`openspec validate --all --strict --no-interactive` 仍
   `44 passed / 0 failed`，输出只多一行 `✓ change/zz-w143-probe`（探针已删）。
2. **`validateChange()` 只有一个调用方，且它刻意不阻塞**。源码 `archive.js:1153`：
   `if (!json) { … const changeReport = await validator.validateChange(changeFile); }`，
   紧接着的注释原文是 `// Proposal validation is informative only (do not block archive).`，
   打印头是 `Proposal warnings in proposal.md (non-blocking):`。`commands/validate.js` 与
   `commands/change.js` 只调 `validateChangeDeltaSpecs()`，**不调** `validateChange()`。
3. **阈值确实存在、但只到提示为止**：`core/validation/constants.js` 里
   `MAX_WHY_SECTION_LENGTH = 1000`，`change.schema.js` 用 `.max(...)` 引用它，
   提示文案即 `CHANGE_WHY_TOO_LONG` —— 也就是我们反复看到的那一行。
4. **存量（用 upstream 自己的 `ChangeParser` 逐条量 139 份归档 proposal）**：
   137 份解析成功、**2 份解析失败**（`2026-10-09-engine-assembly` 缺 What Changes、
   `2026-10-10-w31-golden-set-payload` 缺 Why）；**32 份超 1000**，最长 **2611**
   （`2026-10-12-w70-transient-host-conditions`）。近三轮：W140 = 1171、W141 = 1228、W142 = 601。
   ⚠️ 口径只有这一个：**Why 段的字符数**。此前 memory 里的「W141 2171 / W140 1953」既不是
   段长（1228 / 1171），也不等于整份 proposal 的长度（2951 / 2833），是第三种口径，已作废。

### 裁定：不设判据

三条理由，按分量排：

1. **执行点在依赖里，且它自己选择不阻塞**。上游注释说得很清楚：这条提示之所以存在，
   是为了在归档那一刻给人看一眼，而不是拦人。我们要么改 `node_modules`（vendor，不做），
   要么自造一条**新的**判据面 —— 后者是一个独立的轮次，不是收口。
2. **它守的是阅读性，不是行为**。仓库里其余判据都能回答「不守会坏什么」：取值域坏了数据会腐、
   路由坏了任务会丢、门控坏了测试会静默跳过。Why 段超长的后果是读者费劲，没有一条链路会因此
   产出错误结果 ⇒ **给不出「只有它错时才红」的样本**，这条判据无法证明自己必要。
3. **存量证明它从未被当作硬约束**。32/137 是既有事实。任何回溯式判定都会立刻把已发布的
   历史记录判红，而修法只能是改写历史 —— 那比「不判」坏得多。

### 代价（不付这个，裁定就成了「假装守住了」）

- 上述四条事实写进本 ADR 与 W143 报告 ⇒ 下次有人看到那行提示，不必重测就知道它
  **不阻塞、也不被检查**。
- 本轮的 OpenSpec 变更自己把 Why 段控制在 1000 字符内。这是**样本**，ADR 与报告里
  MUST NOT 写成「已守住」或「已通过」——样本不是判据。

## 后果

- `.gitignore` 多一条带理由的条目；`pending-registration` 的补集判据在本机与 CI 上形状一致
  （都是 0 条），并**仍然刻意不钉** `scanned > 0` —— 理由从「两边不同」换成
  「0 是期望状态」，不是理由消失。那句注释同时订正了一处笔误：它原先说配对物是
  `scanned > 0`，而那条断言在同轮（W135）就因为「输入是工作区状态」被删掉了。
- 三处依赖旧前提的表述分流处置：`pending-registration.test.mjs` 的注释与
  `docs/development-plan.md` 两处**就地订正**（活载体）；W135 报告**追加勘误段**
  （已发布的历史记录）。归档的 W135 变更**保持原样**：它是冻结快照，其「仍在补集里」
  是对当时状态的登记，改它等于篡改历史。
- 凭据扫描域少了 24,796 条路径，且**这是有意为之**。将来若要扩大扫描域，正确方向是让产物
  进仓库（像 `/.nai-diagnostics/`），不是取消忽略。
- Why 段上限继续只是软提示。**若上游哪天把 `validateChange()` 接进 `validate`，
  32 份归档是否会变红取决于它扫不扫 `archive/`** —— 届时 MUST 重测，
  MUST NOT 按本 ADR 的结论推断。
- N4 补的两条断言（`record_candidate` → `1`、`approve` → `6`）与既有三处裸字面量
  （`12`×2、`14`）合起来，写入面四个取值各有字面覆盖；两处注入（`STATUS_PENDING: 1→2`、
  `STATUS_APPROVED: 6→5`）各自只令该用例变红，确认它们具判别性、不是「常量比常量」。
