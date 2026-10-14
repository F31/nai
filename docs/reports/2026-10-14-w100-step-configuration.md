# W100：判定步的**执行配置面**（`matrix` × `if:` × `continue-on-error`）必须由判据读出

- 变更：`openspec/changes/2026-10-14-w100-step-configuration`（`archive` 后真路径）
- ADR：[0078](../adr/0078-step-configuration.md)
- 前置：ADR 0073（CI step 步层闭世界）、ADR 0077（站点层级声明）、W84（站点可达性闭世界）

## 一、目标

`.github/workflows/foundation.yml` 有 **1 个 job、21 个 step**。W95（ADR 0073）把**步集合**管住了：
身份由解析得出、与在册清单双向一致、`kind` 落在闭集里、`local` 要引出真的门控落点、`ci-only` 要
给出成因、`diagnostic` 必须由 `if: failure…` 认出。W97／W98／W99 又分别管住了包集合、脚本消费者、
站点层级。

但每个步的**执行配置**——**跑不跑**（`if:`）、**在哪个平台跑**（`matrix`）、
**失败了会不会被吞**（`continue-on-error`）——**从来没有被任何判据读过**：

```text
workflowStepsOf(text) → [{identity, condition, run}]   ← W95 解析器只产出这三个字段
                        ↑ condition 只被 diagnostic 分支读一次：condition.includes('failure')
matrix / continue-on-error                              ← 解析器根本不产出，判据无从读起
```

⇒ 这不是「有一个洞」，而是**一整个面**没人读。

## 二、缝在哪

### 2.1 探针（改真文件 → 真跑 8 个 portable 套件 → 逐字节还原 → `sha256` 逐字一致）

`.baseline`：8 个 portable 套件合跑 **214 passed / 0 failed**；
`foundation.yml` 的 `sha256 = 2f46d5f52ce8b9af86c6b8947b0c1f012168f411ac50cc78133a68da1a248c1c`。

| 编号 | 改动 | 失败数（8 套件） |
| --- | --- | --- |
| **P-A0** | 基线 | `[0,0,0,0,0,0,0,0]` |
| **P-A1** | 三语言契约套件步的 `if: runner.os == 'Linux'` → `'FreeBSD'` | `[0,0,0,0,0,0,0,0]` **零信号** |
| **P-A2** | `matrix.os: [ubuntu-24.04, windows-latest]` → `[windows-latest]` | `[0,…]` **零信号** |
| **P-A3** | `local` 门控步加 `continue-on-error: true` | `[0,…]` **零信号** |
| **P-A4** | 删掉 `Install Linux C++ protobuf runtime` 的平台条件 | `[0,…]` **零信号** |
| **P-A6** | `local` provider 步加 `continue-on-error: true` | `[0,…]` **零信号** |
| **P-B1** | 判定步的 `run` 换成 `cargo xtask check --registry`（标签不变） | `[0,…]` 形状级判定放行 |
| **P-A5** | 删掉诊断步的 `if: failure()`（**对照组**） | `workflow` 套件 **2 条红** ⇒ W95 已兑付 |
| 还原 | 逐字节还原（`read_bytes`/`write_bytes` + 断言 `\r` 计数不变） | `RESTORE OK`，`sha256` 逐字一致 |

P-A5 是对照组：它证明上面那些「零信号」**不是**因为判据整体失效，而是因为它们落在一个
**从未被判据覆盖的面上**。

### 2.2 最危险的一个是矩阵

`os: [a, b]` → `[b]` 之后，两条带 `if: runner.os == 'Linux'` 的步**永远不会跑**——其中一条是
**三语言契约套件**。而「21 个 step 都在册、身份都在、`kind` 都在闭集里、`localEvidence` 都在、
`ci-only` 成因都在闭集里」——**每一格都还是绿的**，CI 也照样绿。

### 2.3 三个陷阱

1. **「写了条件」≠「会跑」**：`if: runner.os == 'macOS'` 语法正确，但当前矩阵里没有 macOS
   ⇒ 该步永不跑。只问「有没有 `if:`」会把它放过去——**字段级看不见的那一格**。
2. **失败语义与角色必须一致**：`local`/`ci-only` 带上 `continue-on-error` ⇒ 判定可以失败而
   CI 保持绿（**装饰**）；反过来 `diagnostic` 缺 `continue-on-error` ⇒ 诊断自身的问题会
   **替换**它所转译的那次失败的结论。仓库里那句注释（「保证诊断自身的问题不会掩盖或替换原始
   失败的结论」）是**散文**——散文不执行任何东西。
3. **成因与条件必须互相兑现**：`platform-toolchain` 若没有平台限制，「它需要特定平台」在 CI 上
   没有任何可见后果；反过来给与平台无关的成因加平台条件，会让该步在别的平台**静默不跑**。

### 2.4 ⚠️ 探针方法论：本轮踩到的两条陷阱 + 印证一条旧教训

1. **Python `Path.write_text` 在 Windows 上把 `\n` 翻成 `\r\n`** ⇒ 破坏测试夹具里所有含 `\n` 的
   字面锚点，探针**自己制造红**。首轮实测 P-A1/A2/B1 得 `[0,0,0,4,0,1,0,0]`，排查后确认是行尾
   翻译、不是信号。**修法 = `read_bytes`/`write_bytes` 逐字节写入，并断言「`\r` 计数与原文一致」。**
   重跑后三项全 `0`。
2. **某些步名被测试当作夹具锚点**：实测 `Kernel registration and binding` 在
   `workflow.test.mjs` 里出现 **11** 次、`Install Linux C++ protobuf runtime` **1** 次。
   对它们注入会**扰动夹具** ⇒ 假红。P-A6 原用前者得到 1 条假红，改用零命中的
   `API Provider transport and retry (mock server; no paid calls)` 后为 `0`。
   **修法 = 探针选「零命中」的步（先统计命中次数再选）。**
3. **（印证旧教训）「逐字还原」只有在「起点干净」被断言之后才有意义**：本轮的 `restore()` 先
   断言 `git diff --quiet` 干净，再取基线字节；结束时用**备份拷回**并双向核对
   `sha256` + `git diff --quiet`。

## 三、判据设计

新 `scripts/contracts/step-config.mjs`，5 组 8 条：

| 编号 | 判据 | 判定域 |
| --- | --- | --- |
| R1 | 在册步的**执行平台集合** MUST 非空 | `matrix`（或字面 `runs-on`）× `if:` 静态求值 |
| R2a | `local`/`ci-only` MUST NOT 带真值 `continue-on-error` | job 视图的 `continueOnError` |
| R2b | `diagnostic` MUST 带真值 `continue-on-error` | 同上 |
| R3a | `platform-toolchain` MUST 被**真子集**平台条件兑付 | `CI_ONLY_REASONS` × 平台族集 |
| R3b | 其余三因 MUST NOT 被平台限定 | 同上（**反向**） |
| R4 | `matrix`/`if:`/`continue-on-error`/runner 标签/`runner.os` 取值 MUST 落在闭集里 | 三张表 |
| R5 | `run` 命中平台专属命令 ⇒ 平台集 MUST ⊆ 该形状支持集且非空 | `PLATFORM_COMMAND_SHAPES` |
| R6 | 非空洞：规则表每格 MUST ≥ 1 实例、判定集 MUST 非空 | 三张规则表 + `judged` |

三张闭世界表与**刻意不同**的非空洞要求：

| 表 | 取值 | 域外 | 非空洞 |
| --- | --- | --- | --- |
| `CONDITION_SHAPES` | `platform-os` / `failure` | 未知形状 ⇒ 具名 | 每格 MUST ≥ 1 实例（**规则表**） |
| `RUNNER_OS_VALUES` | `Linux` / `Windows` / `macOS` | 未知取值 ⇒ 具名 | **不要求**每值有实例（**字典**） |
| `RUNNER_LABEL_PREFIXES` | `ubuntu-` / `windows-` / `macos-` | 不可分类 ⇒ 具名 | 只要求矩阵里出现过的标签可分类（**字典**） |
| `PLATFORM_COMMAND_SHAPES` | `posix-privilege-escalation`（`sudo`，支持 `Linux`+`macOS`）／`debian-package-manager`（`apt-get`，仅 `Linux`） | 域外 ⇒ 具名 | 每格 MUST ≥ 1 实例（**规则表**） |

**两层取值域拒否合并不了一处**（决策 3）：

| 情形 | 谁拦 | 为什么不是另一条 |
| --- | --- | --- |
| `runner.os == 'FreeBSD'` | 取值域 | runner 从不报 `FreeBSD` ⇒ **读不懂的取值** |
| `runner.os == 'macOS'`（矩阵里没有） | R1（集合为空） | 取值合法、条件可读，但对**当前矩阵**恒假 |
| `matrix.os: []` | 取值域 | 空矩阵 ⇒ 零个 job ⇒ **声明本身空洞** |

## 四、实现

| 文件 | 改动 |
| --- | --- |
| `scripts/contracts/step-config.mjs` | **新增 356 行**（判据） |
| `tests/contracts/step-config.test.mjs` | **新增 475 行 / 15 个用例** |
| `scripts/contracts/workflow-steps.mjs` | **+37 / −5**：拆出 `workflowJobsOf`，`workflowStepsOf` 增补 `matrix`/`continueOnError` |
| `scripts/contracts/test-reachability.mjs` | **+7**：站点 `xtask-engineering-step-config`（`portable`） |
| `tests/registry.test.mjs` | **+5 / −3**：逐字站点清单 12 → 13（含注释里的轮次说明） |
| `xtask/src/lib.rs` | **+7**（1 行登记 + 6 行注释）：Rust **零行为改动** |

**被审对象一个字符未动**：`.github/workflows/foundation.yml`（`sha256` 仍为 `2f46d5f5…8c1c`）
与 `WORKFLOW_STEPS` 的既有 **21 条**。W95 的 `{identity, condition, run}` 语义不变
⇒ `tests/contracts/workflow.test.mjs` 的 37 个用例**全部不变**（实测 13/13 通过）。

**解析器只留一份定义、产出两个视图**：

```js
workflowJobsOf(text)   → [{id, matrix, runsOn, steps: [{identity, condition, run, continueOnError}]}]
workflowStepsOf(text)  → [{identity, condition, run, job, matrix, continueOnError}]   ← 扁平派生
```

`matrix` 在扁平视图里对同 job 的步是**同一个对象引用**（不是各自拷贝）——F9 用例逐字钉住这一点，
并逐字对账「扁平视图 == job 视图的派生」。

## 五、非空洞验证

**注入 V1–V11 全部真改文件、真跑 9 个套件、逐字节还原**（`foundation.yml` `sha256` 前后一致
`2f46d5f5…8c1c`），**11 个探针全部被至少一条判据注意到**：

| 探针 | 改动 | 命中判据（具名） | 边界（W84/W95/W96/W97/W98/W99/W100） |
| --- | --- | --- | --- |
| **V1** | 矩阵 → `[windows-latest]` | R1 ×2 | `0/0/0/0/0/0/3` |
| **V2** | 门控步 +`continue-on-error` | R2a | `0/0/0/0/0/0/3` |
| **V3** | 诊断步 −`continue-on-error` | R2b | `0/0/0/0/0/0/4` |
| **V4** | `runner.os` → `'FreeBSD'` | R4（取值域） | `0/0/0/0/0/0/2` |
| **V5** | 删 Linux C++ 安装步的平台条件 | R5 ×2 | `0/0/0/0/0/0/2` |
| **V6** | `spec:validate` +Linux 条件 | R3b | `0/0/0/0/1†/0/2` |
| **V7** | 删三语言契约套件的平台条件 | R3a | `0/0/0/0/0/0/2` |
| **V8** | 诊断步 `if: failure()` → `always()` | R4（形状）+ R6 | `0/2/0/0/0/0/3` |
| **V9** | 矩阵多一轴 | R4（轴闭世界） | `0/0/0/0/0/0/2` |
| **V10** | `continue-on-error: ${{ matrix.os }}` | R4（开关形状） | `0/0/0/0/0/0/3` |
| **V11** | 步改名（**边界探针**） | **W100 沉默**（返回 `[]`） | `0/4/0/1†/0/0/1†` |

**† = 夹具前提被扰动的假红，不是判据判定**（见 §2.4 陷阱 3 / §九 4）。V11 是本轮的**反向边界
证据**：`stepConfigErrors` 在该改动下返回 `[]`（W100 确实不读步身份），而 W95 的步集判据红 4 条。

**独有覆盖**：

- **V1–V10**：W100 全部响（具名）；其中只有 **V8** 让 W95 也响——诊断步的条件正是 W95 的判定域。
  W84／W96／W97／W98／W99 在 V1–V10 上 **0 红**。
- **V11（边界探针）**：W100 **沉默**（`stepConfigErrors` 返回 `[]`），W95 的步集判据红 **4** 条
  ——这是本轮的**反向边界证据**：W100 不读步身份。
- **† 假红共 3 处**：V6 的 `npm-scripts` **1**（W98）、V11 的 `gate-coverage` **1**（W97）
  与 `step-config` **1**。三处的根因都是**夹具前提被扰动**（见 §2.4 陷阱 3），
  **不是判据在响**——V11 上 `stepConfigErrors` 返回 `[]` 即为明证。

V8 上 W95 与 W100 **同时**红，但**消息与判定域都不同**——W95 判「该步的角色与条件是否一致」，
W100 判「条件形状表是否还有实例」（**冗余，不是重复定义**）。

**每条判据都有具名注入**：15 个用例里 F0–F9 覆盖 R1–R6 的每个方向（含反向不红）、合成世界与
真实仓库两路；`patch()` 自带「改不到就断言失败」，MUST NOT 让 no-op 注入冒充证据。

## 六、门控与 validate

**本机门控**：`cargo xtask check --profile full`（`engineering_checks` + `frontend::check`）。

- **第一次 `EXIT=1`**，折在**最后一步** `cargo test --locked --package nai-xtask --lib` ——
  `LINK : fatal error LNK1104: 无法打开文件 "E:\nai-build-target\debug\deps\nai_xtask-01d6cdd49aa03c60.exe"`。
  按既定判据**逐目标复跑**：该目标单跑 **52 passed / 0 failed（`EXIT=0`）**
  ⇒ **非代码缺陷**（宿主瞬态，Windows 侧锁住正在执行的镜像／并发重链）。
  **处置 = 限并发（`CARGO_BUILD_JOBS=1`）重跑，未改一字符代码。**
- **重跑 `EXIT=0`（4m48s）**：`grep -c '^test result:'` = **91**（其中 18 条 `Doc-tests`）、
  `^     Running ` = **74**（= 91 − 18 + 1 包装行）、`passed` 合计 **862**、`failed` **0**；
  日志内 `LNK1104` / `error[` / `out of memory` 命中各 **0**。
  ⇒ **W100 = 91/862，与 W95／W96／W99 逐项相同**（本轮 Rust 零行为改动，只加 1 行登记 + 6 行注释），
  这是「未误改别处」的旁证。

**两条 `openspec validate`**（都跑，互不替代）：

- `openspec validate 2026-10-14-w100-step-configuration --strict --no-interactive` → **valid**（归档前）。
- `openspec validate --all --strict --no-interactive` → **43 个规格，0 failed**（归档后复跑）。
- `openspec validate --all --strict --archived --no-interactive` → **98，0 failed**
  （W99 的 97 → **98**，每归档一次 +1）。
- ⚠️ `archive --yes` 报了 `proposal.md` 的 **Why 段超 1000 字符**（实测 **1552** 字符）
  的**非阻塞警告** `CHANGE_WHY_TOO_LONG`；归档后严格校验仍 **0 failed**（与 W83／W85 实测一致，
  该警告**按轮而定、不照抄**）。

## 七、CI

（本节在推送后按**单次低频观测**回填：唯一查询参数 + `Cache-Control: no-cache` + 回读
`head_sha` == 推送 SHA。）

## 八、遗留

1. **「步身份 ↔ 判定目标」的绑定仍未管**（P-B1）：把一个判定步的 `run` 换成另一条**同样像判定**
   的命令（`cargo xtask check --registry`）**零信号**。ADR 0075 的 V6 只做**形状级**判定
   （「这是不是一条判定命令」），**不绑定**「这一步该跑哪个目标」。这是**独立的**一层。
2. **`if:` 表达式求值语义未实现**：只做**闭形状**分类（见 §九 1）。
3. **`infra` 步的 `continue-on-error` 未判**：环境准备失败会让下游判定步失败，是**自曝**而非静默
   （§九 2）。
4. **同族矩阵下的严格方向**：若矩阵的所有腿属于同一 OS 族，`platform-toolchain` 会因「CI 侧无
   可见后果」被判红。这是**有意的严格**，但它在「全 Linux 矩阵 + 平台工具链成因」这一配置上
   会要求人改签——该配置在本仓库不存在。
5. W90 §八 1/3/4/5、W91 跨语言键级消费、W92 基线收录、W93 锚点唯一性、W94 `needs_external`
   审计、W95 `ci-only` 成因、W96 五项、W97 六项、W98 §八 1/2/7、W99 §八 1/2/3 **均未动**。
   （其中 **W96 §八 3** 已由 W97 的 V4 顺带兑付，本轮复核确认。）

## 九、如实披露

1. **`if:` 的求值是形状级的**：只认 `runner.os == '<os>'` 与 `failure()`。其余形状
   （`always()`、`success()`、`github.event_name == …`、`!cancelled()` …）一律**具名拒绝**。
   拒否方向落在**假阳性**（要求人显式归类），不是假阴性。
2. **R5 是形状级**，且会扫到 `run` 文本里的**注释**（一行 `# 需要 sudo` 会让该步被要求平台限定）
   ⇒ 假阳性方向。
3. **`sudo` 的支持集按实测的平台事实写**：`['Linux','macOS']`，不是 `['Linux']`。写成 `['Linux']`
   会在 macOS 矩阵下误报一个**正确**的配置。
4. **`run` 扫整段而非首行**：`sudo apt-get update && sudo apt-get install …` 里 `apt-get` 在 `&&`
   之后 ⇒ 只用首行会让 `debian-package-manager` 这一格零实例，与「每格 MUST ≥ 1 实例」冲突。
5. **一处刻意的非对称**：一个在册步在 workflow 里**不存在**时本判据**沉默**——那是 W95
   「a registered step that vanished」的判定域。
6. **规则表与字典的非空洞要求刻意不同**（§三）：要求「所有平台都出现在矩阵里」等于把字典
   当断言。
7. **本轮跨套件红必须先确认前提**：V6/V11 除本判据外还打红了别的套件（共 3 处，见 §五），
   根因是**那些用例的 `replace` 锚点已被探针改掉**（注入变成 no-op ⇒ 断言前提没了），
   **不是判据在响**——V11 上 `stepConfigErrors` 返回 `[]` 即为明证。
   这一类与 §2.4 的陷阱 1/2 同族：**探针会扰动夹具**。
