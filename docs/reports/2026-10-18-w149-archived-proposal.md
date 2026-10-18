# W149 报告：归档记录的结构判据与具名台账

立项依据：W143 §八第 1 条（悬了 5 轮）。范围与本轮改动见
`openspec/changes/archive/2026-10-18-w149-archived-proposal/`，裁定见
[ADR 0123](../adr/0123-archived-proposal-structure.md)。

## 一、勘定与现测

### 1.1 归档语料有多大、有多少不合规

对 `openspec/changes/archive/` 全部目录逐份读 `proposal.md`，按行首 `## ` 切段，标题归一化
（大小写不敏感、`Changes` 的 `s` 可选）：

| 组 | 值 |
| --- | --- |
| 归档变更目录 | **145**（每个目录都有 `proposal.md`） |
| 有 `## Why` + `## What Changes` | **143** |
| 缺 `## What Changes` | **1**：`2026-10-09-engine-assembly`（写的是 `## What`；Why 存在且 235 字符） |
| 两段都缺 | **1**：`2026-10-10-w31-golden-set-payload`（标题 `为什么`/`变更`/`影响`/`否决方案`） |
| 段存在但正文为空 | **0** |

标题变体的实测分布：`Why` 只有一种写法；`What Changes` 138 份、`What changes` 5 份
⇒ **归一化只需大小写不敏感**，`Changes` 的 `s` 可选是为了不把 5 份正常文档判红。

### 1.2 现有门控对这条是否零信号

这是本轮立项的**唯一前提**，所以单独测：`openspec validate --archived` 的输出里，两份缺段文档
分别是 `✓ change/2026-10-09-engine-assembly` 与 `✓ change/2026-10-10-w31-golden-set-payload`，
总计 **145 passed, 0 failed**。

⇒ `--archived` 判的是「归档过的东西能不能解析」，不是「它有没有把话说完」。而 `archive.js` 侧的
`validateChange()` 是 **non-blocking**（ADR 0117）：一份新归档缺 Why，只在归档那一刻打一行警告，
之后没有任何门控会再问 —— 缺口被「归档」这个动作**永久固化**。

**所以这不是「再加一条判据」，而是补掉一次「警告过后没人问」的沉默。**

## 二、判据形态

三条决定（理由全文见 ADR 0123）：

1. **判全部归档，不按时间划界。** 「本判据落地之后归档的才算」是一条按日期开的豁免：不具名、
   不自清理，且可以把目录名改早一天绕过。
2. **归一化只到大小写与 `Changes` 的 `s`。** `## What` 与 `## 为什么` 不被接受——它们正是台账
   存在的理由，不是归一化该放行的样本。边界**两侧**都钉了断言。
3. **空段等于缺段，措辞不同。** 今天 0 份，判据仍要判：留着标题删掉句子，是让缺口看起来像答过了。

实现落点：

| 名称 | 作用 |
| --- | --- |
| `REQUIRED_SECTIONS` | 标题归一化的**唯一出处**（`/^why$/i`、`/^what\s+changes?$/i`） |
| `PROPOSAL_EXCEPTIONS` | 具名台账（2 条，`sinceBaseline: V3.11`） |
| `archivedChangeNames` | 域：归档目录下**任意**已跟踪文件 → 变更名（不只 `proposal.md`） |
| `sectionsOf` | 行首 `## ` 切段，同名取第一处，正文 `trim` |
| `archivedProposalErrors` | 纯函数，入参 `{changes, exceptions}` 可注入；返回 `{errors, changesChecked, exceptionsUsed}` |

七类具名失败：缺段、空段、归档目录无 `proposal.md`、台账相称不符、台账陈旧（文档已补齐）、
台账无主体、域为空。

## 三、台账的三项纪律（本轮唯一的返工点）

台账条目 `{change, missing, sinceBaseline, reason}`：**具名**（变更必须真的在域内）、
**相称**（`missing` == 实测缺口）、**自清理**（补齐后条目仍在 ⇒ 红）。

**返工记在这里**：第一版把「缺段」的报错**立刻 push**，再单独处理台账 —— 结果是台账条目根本
不抑制它盖住的那几条，真实语料基线直接红了 3 条（`2026-10-09` 一条 + `2026-10-10` 两条）。
那不是判据的严格，是**例外没有生效**：例外不抑制报错，它只是把红字旁边加了一行注释。
改成「先把缺失与措辞收进 `messages`，有台账就只做相称校验、不 push `messages`」之后才绿。

⚠️ 相称不符时**只报一条**（并同时给出两边），不再叠上 `messages`：否则「台账写错了」与
「文档真缺段」会看起来是同一件事，而这里要判的恰恰是前者。

## 四、判据与注入

运行 `node --test tests/contracts/archived-proposal.test.mjs`（3 条），每次注入后按 sha256 逐字节还原
（基线指纹 `316d5cfd8f8ba9a8df508a64a2d577803296c2e884f51b2a120ed8526e1192a8`，还原后 3/3 绿、工作区干净）：

| 注入 | 内容 | 结果 |
| --- | --- | --- |
| W1 | 关掉台账查询（`const exception = undefined`） | **2 红**（`#1` `#2`）：语料基线与台账用例同时红 |
| W2 | 归一化放宽成 `/^what$/i` | **3 红 / 0 绿**：`engine-assembly` 变合规 ⇒ 台账陈旧；其余 `## What Changes` 文档全变缺段 |
| W3 | 去掉空段判据（`if (found.body === '')` → `if (false)`） | **只红 `#2`** —— 空段恰是它那一层 |
| W4 | 域只认 `proposal.md`（`rest.endsWith('/proposal.md')`） | **只红 `#3`** —— 「目录在、proposal 不在」恰是它那一层 |

W3 / W4 的「只红一条」是本轮最想拿到的性质：每条缺陷都有一个**只有它错时才红**的样本，
而不是所有注入都红成一片。

**既有七层判据在新增站点后仍全绿**（改动 `xtask/src/lib.rs` 与站点表会同时惊动它们）：

| 判据 | 结果 |
| --- | --- |
| `pending-registration` | 11 / 0 |
| `tier-payment` | 22 / 0 |
| `gate-coverage` | 34 / 0 |
| `step-config` / `step-target` | 16 / 0、16 / 0 |
| `step-overlap` / `workflow-triggers` | 12 / 0、15 / 0 |

## 五、诚实边界

- **只判 `proposal.md`。** 归档里的 `tasks.md` / `specs/` 结构未判：`--archived` 会报未勾任务，
  但段结构与 Requirement 形态没有独立判据。
- **未跟踪的归档目录不在域内。** 域来自 `.git/index`（W75）；与 W135 的 `pending-registration`
  是同一条边界，两者**不互相替代**（一个管「有没有登记」，一个管「登记过的有没有把话说完」）。
- **台账的 `sinceBaseline` 只校验非空**，不校验「是否等于当前基线」——那需要「基线版本从哪读」
  的唯一出处，属独立一轮。
- **不判段内长度**。Why 段的 1000 字符上限仍按 ADR 0117 保持无门控；本轮只判「有没有」与「空不空」。
- **不改历史记录**。两份例外留在台账里，判据不要求改已发布的 proposal；「补段」这件事本身
  仍然没人做，只是从「沉默」变成「具名且会自清理」。

## 六、门控

`cargo xtask check --profile full`（`CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1`）→ **第 3 次 `EXIT=0`**，
2 分 33 秒。

| 项 | 值 |
| --- | --- |
| 门控内 `Compiling` | **0**（第 2 次跑时是 1——本轮改了 `xtask/src/lib.rs`） |
| 门控内 `LNK1104` | **0**（第 2 次跑时是 **1**，见下） |
| Rust | **1147 / 0**（102 条 `test result` 行，与 W148 完全一致 ⇒ 本轮 Rust 零改动） |
| Node | **387 / 0**（**16** 组；W148 是 384 / 15 组 ⇒ **+3 用例、+1 组**，正是本轮新增的那份测试） |
| `validate --all --strict` | 归档前 **44 / 0**；归档后 **43 / 0**（变更移出活跃集，−1） |
| `validate --archived` | 归档后 **146 / 0**（W148 是 145，+1 即本轮归档的变更） |

**新用例在门控里逐条跑过**，不是从总数倒推：

```
# Subtest: every archived proposal still states why it exists, and the ledger matches the corpus
ok 1 - …
# Subtest: a missing or empty section is named, and the ledger cannot cover what it does not describe
ok 2 - …
# Subtest: the archive domain is every tracked file under it, so a change with no proposal is still in scope
ok 3 - …
```

**判据的第一个受审对象就是本轮自己**：归档后 `--archived` 从 145 变 146，而判据在同一时刻仍为绿
⇒ 本轮自己的 `proposal.md` 通过了它自己新增的那条规则（`## Why` + `## What Changes` 都在）。

### 第 1、2 次 `EXIT=1` 的两次原因（都不是代码缺陷）

| 次数 | 红在哪 | 成因 | 处置 |
| --- | --- | --- | --- |
| 1 | `tests/registry.test.mjs` 两条（`#50` `#52`） | 站点表是**逐字钉住**的（新增站点必红），且 `#52` 要求「站点 MUST 覆盖到**已跟踪**的文件」——新测试文件当时还没 `git add` | 把新 id 加进逐字清单（并把注释里的「十七个」改成「十八个」），同时 `git add` 新文件 |
| 2 | `cargo test -p nai-xtask --lib` 链接 | `LNK1104: 无法打开文件 …nai_xtask-07bc857a8dd18697.exe`，即 W147 §八第 1 条那条宿主瞬态 | **不改代码**：单独 `cargo test -p nai-xtask --lib --no-run` 一次即成功（5.6 s）⇒ 判为宿主瞬态；第 3 次跑 `EXIT=0`、`Compiling 0`、`LNK1104 0` |

第 1 次是**判据在正常工作**：它正是 W95/W134/W135 那条「新增测试文件而不登记站点/没进索引 ⇒
闭世界与逐字清单同时红」的现场复现。第 2 次则复现了 W147 登记的那条宿主瞬态——两次都**没有**
改动被测代码或放宽判据。

## 七、CI 观测

提交 **`c838982`**（14 文件 / +876 −2）→ run **`39132041750`**（`Engineering foundation`，第 245 次），
`completed / success`，`head_sha` 完整 40 位一致（`c8389824e5e12f823121de0c201fdfe6dee24d79`）。
判结论三件齐（唯一查询参数 `cb=<纳秒>` + `Cache-Control: no-cache` + 回读 `head_sha`）；完成态由
100 s 间隔的有界轮询在**第 7 次**看到（`created 13:33:08Z` → `updated 13:45:26Z`，约 12 分 18 秒），
随后用 `/jobs` 与 `/runs/{id}` 各查一次确认（**完成态共 3 次查询**，结论与 `head_sha` 全同）。

| 平台 | 状态 | 步骤记录数 | 编号 | 被跳过 |
| --- | --- | --- | --- | --- |
| `foundation (ubuntu-24.04)` | **success** | 20 | `1–16` 后跳 `30–33` | `11` |
| `foundation (windows-latest)` | **success** | 20 | `1–16` 后跳 `30–33` | `8` `11` `14` |

两平台各 20 条记录、编号 `1–16` 后跳 `30–33`，跳过集合与 W148 逐条相同 ⇒ 本轮的改动没有改变
执行面形状，只改变了第 10 步里跑的东西（`check --profile full` 多了一步纯静态 Node 判据）。
第 10 步与第 16 步（`openspec validate --archived`）两平台均 **success**。

**干净 checkout 上的意义**：新增判据的输入是 `.git/index` 与归档目录文本，两者都在仓库里
⇒ CI 侧的域与本机一致（本机 145 → 归档后 146 份，CI 检出的是同一份提交）。这也意味着
「本机绿 / CI 红」在这条判据上没有机制性空间：没有一条输入取自工作区残留。



## 八、具名登记（本轮发现但不做）

1. **归档 `tasks.md` / `specs/` 的结构无判据**（ADR 0123 残余 1）。
2. **`sinceBaseline` 不校验与当前基线一致**（残余 3）：需要「基线版本唯一出处」，否则这行只是注释。
3. **W147 §八第 1 条（`LNK1104` 宿主成因）仍未判**：需动宿主配置做对照实验，本轮仍没碰。
4. **W148 §八其余 3 条**（`openspec/` 域外、散文域外、`docs/contracts/README.md` 在域内无处方）
   仍在列；W145 / W146 §八 其余各条同样在列，本轮没有接手。
5. **`archive.js` 的 non-blocking 语义仍在**：本轮只在**提交后**补判，没有让归档动作本身阻塞。
   要改它得改外部工具的调用方式（`pnpm exec openspec` 的退出码不可控），属另一条口径。
