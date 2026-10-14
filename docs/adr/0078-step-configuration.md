# ADR 0078：判定步的**执行配置面**必须由解析出的平台集合兑付

- **日期**：2026-10-14
- **状态**：已接受（Corresponding change：`openspec/changes/2026-10-14-w100-step-configuration`）
- **关联**：ADR 0073（CI step 的步层闭世界）、ADR 0077（站点层级声明）、ADR 0074（本机命令面）、
  W84 的「登记站点」闭世界、W97 的「集合级而非字段级」、W99 的「配对称谓 MUST 由解析得出」

## 背景

W95（ADR 0073）给 `.github/workflows/foundation.yml` 的 step 集补了**步层闭世界**：步的身份由
解析得出、与在册清单双向一致、`kind` 落在闭集里、`local` 要引出真的门控落点、`ci-only` 要给出
成因、`diagnostic` 必须由 `if: failure…` 认出。W97/W98/W99 又分别管住了包集合、脚本消费者、
站点层级。

但每个步的**执行配置**——**跑不跑**（`if:`）、**在哪个平台跑**（`matrix`）、
**失败了会不会被吞**（`continue-on-error`）——**从来没有被任何判据读过**：

```text
workflowStepsOf(text) → [{identity, condition, run}]   ← W95 解析器只产出这三个字段
                        ↑ condition 只被 diagnostic 分支读一次：condition.includes('failure')
matrix / continue-on-error                              ← 解析器根本不产出，判据无从读起
```

⇒ 这不是「有一个洞」，而是**一整个面**没人读。实测（W100 探针，改**真文件**、真跑 8 个
portable 套件、逐字节还原、`sha256` 逐字一致）：

| 编号 | 改动 | 结果 |
| --- | --- | --- |
| **P-A1** | 三语言契约套件步的 `if: runner.os == 'Linux'` → `'FreeBSD'` | `[0,0,0,0,0,0,0,0]` **零信号** |
| **P-A2** | `matrix.os: [ubuntu-24.04, windows-latest]` → `[windows-latest]` | `[0,…]` **零信号** |
| **P-A3** | `local` 门控步加 `continue-on-error: true` | `[0,…]` **零信号** |
| **P-A4** | 删掉 `Install Linux C++ protobuf runtime` 的平台条件 | `[0,…]` **零信号** |
| **P-A6** | `local` provider 步加 `continue-on-error: true` | `[0,…]` **零信号** |
| **P-B1** | 判定步的 `run` 换成 `cargo xtask check --registry`（标签不变） | `[0,…]` 形状级判定放行 |
| **P-A5** | 删掉诊断步的 `if: failure()`（**对照组**） | `workflow` 套件 **2 条红** ⇒ W95 已兑付 |

P-A2 最能说明问题：矩阵缩成一个平台后，**两条 Linux-only 步永远不会跑**（其中一条就是三语言
契约套件），而「21 个 step 都在册、身份都在、`kind` 都在闭集里」——**每一格都还是绿的**，
CI 也照样绿。P-A5 是对照组：它证明那些「零信号」不是因为判据整体失效，而是因为它们落在一个
从未被判据覆盖的面上。

三个陷阱在这一层出现：

1. **「写了条件」≠「会跑」**（字段级看不见的那一格）：`if: runner.os == 'macOS'` 是一段语法正确
   的条件，但当前矩阵里没有 macOS ⇒ 该步**永不跑**。只问「有没有 `if:`」会把它放过去。
2. **失败语义与角色必须一致**：`local`/`ci-only` 步带上 `continue-on-error` ⇒ 判定可以失败而
   CI 保持绿（**装饰**）；反过来 `diagnostic` 步缺 `continue-on-error` ⇒ 诊断自身的问题会
   **替换**它所转译的那次失败的结论。仓库里那句注释（「保证诊断自身的问题不会掩盖或替换原始
   失败的结论」）是**散文**——散文不执行任何东西。
3. **成因与条件必须互相兑现**：`ci-only` 的 `platform-toolchain` 若没有平台限制，「它需要特定
   平台」在 CI 上没有任何可见后果；反过来给一个与平台无关的成因（`host-subprocess` 等）加上
   平台条件，会让该步在别的平台**静默不跑**。

## 决策

### 决策 1：解析器**只留一份定义**，产出**两个视图**

```
workflowJobsOf(text)   → [{id, matrix, runsOn, steps: [{identity, condition, run, continueOnError}]}]
workflowStepsOf(text)  → [{identity, condition, run, matrix, continueOnError, job}]   ← 扁平视图
```

`workflowStepErrors`（W95）继续用扁平视图；`stepConfigErrors`（本轮）用 **job 视图**——
它需要 `matrix`，而 `matrix` 是 **job 的属性**，不是 step 的。两个视图来自**同一次** `YAML.parse`：
两份解析器一定会漂移（「同一件事 MUST NOT 两处定义」）。

`matrix` 在扁平视图里对同 job 的步是**同一个对象引用**，不是各自拷贝：拷贝会让「同一个矩阵」
变成 21 份可分别篡改的副本。

W95 既有的 `{identity, condition, run}` 语义**一字不动**，新字段是**增补** ⇒ W95 的 37 个用例
全部不变（实测）。

### 决策 2：可达性是**集合级**的，由 `matrix` × `if:` 静态求值

```
platformLegsOf(step, job) → os[]           // 该步真正会跑的平台腿
  legs = platformsOf(job)                  // matrix.os 的取值，或字面 runs-on
  if: ''                → legs
  if: runner.os == 'X'  → legs 里 OS 族为 X 的那些
  if: failure()         → legs             // 失败条件不筛平台
```

判定 MUST 是集合级的（「执行平台集合**非空**吗」），MUST NOT 是字段级的（「`if:` 写了吗」）。
P-A2 与「`runner.os == 'macOS'`」这两种情形，字段级都看不见。

### 决策 3：取值域拒否**分两层**，各自具名

| 情形 | 谁拦 | 为什么不是另一条 |
| --- | --- | --- |
| `runner.os == 'FreeBSD'` | 取值域闭世界 | GitHub runner 从不报 `FreeBSD` ⇒ 这是**读不懂的取值** |
| `runner.os == 'macOS'`（矩阵里没有） | 可达性（集合为空） | 取值合法、条件可读，但它对**当前矩阵**恒假 |
| `matrix.os: []` | 取值域闭世界（空矩阵 ⇒ 零个 job） | 连「不跑」都算不上，是**声明本身空洞** |

两层的输入不同（一层看取值域，一层看矩阵），**合并不了一处**：合并后错误消息就分不清
「你写了个不存在的平台」与「你的矩阵里没有这个平台」。

### 决策 4：失败语义**双向**

```
kind ∈ {local, ci-only} → continue-on-error MUST NOT be truthy   // 判定的结论必须能红
kind === 'diagnostic'   → continue-on-error MUST be truthy       // 诊断的问题不得替换结论
kind === 'infra'        → 不判（环境准备失败会让下游判定步失败，是**自曝**而非静默）
```

`diagnostic` 那一半不是过度设计：该步**只在失败时跑**（W95 已用 `if: failure` 钉住），
它的工作是**转译**已有失败；如果转译器本身能失败并让 CI 因为**它的**原因变红，
「CI 红」这件事的**因果**就被替换了。

### 决策 5：成因与条件**双向**兑现，分派由成因表定、MUST NOT 写死步名

```
reason === 'platform-toolchain' → 执行平台族集 MUST 是 job 平台族集的**真子集**且非空
reason ∈ 其余三因                → 执行平台族集 MUST 等于 job 平台族集（不得被平台限定）
```

与「分派 MUST 由类别表定」同纪律：新增一个 `ci-only` 原因时，`CI_ONLY_REASONS` 会先红
（W95 的闭世界），归类者必须同时回答「它靠什么兑付」。**「平台限制」不是任何一类 `ci-only`
的通用装饰**——`host-subprocess` 是宿主能不能派生子进程的能力问题，与 runner 的 OS 无关。

比较用 **OS 族**（`Linux`/`Windows`/`macOS`）而不是标签计数：`runner.os == X` 筛的就是族。
若矩阵的所有腿属于同一族，`platform-toolchain` 会因「CI 侧无可见后果」被判红——**有意的严格**
（该配置在本仓库不存在，见「如实披露」）。

### 决策 6：平台专属命令用**形状**判定，且支持集按**实测的平台事实**写

```
{id: 'posix-privilege-escalation', supported: ['Linux','macOS'], pattern: /(?:^|[\s;&|])sudo\s/}
{id: 'debian-package-manager',     supported: ['Linux'],        pattern: /(?:^|[\s;&|])(?:apt-get|apt)\s/}
```

两条决定值得写下来：

- **`sudo` 的支持集不是 `['Linux']`**：macOS runner 也有 `sudo`（只有 Windows runner 没有）。
  把支持集写成 `['Linux']` 会在 macOS 矩阵下误报一个**正确**的配置——**假阳性虽安全，
  但它花在一次误报上**。形状的支持集 MUST 按实测的平台事实写。
- **扫 `run` 的整段文本，不是首行**：`sudo apt-get update && sudo apt-get install -y libprotobuf-dev`
  里 `apt-get` 在 `&&` 之后 ⇒ 只用首行会让 `debian-package-manager` 这一格**零实例**，
  与「每个形状 MUST 有实例」的非空洞断言冲突。

与 ADR 0073 的 `JUDGE_COMMAND_SHAPES` 同一族（都验**命令形状**，不验语义），但断言的是
**平台可得性**。

### 决策 7：配置面闭世界**三张表**，非空洞要求**刻意不同**

| 表 | 取值 | 域外 | 非空洞 |
| --- | --- | --- | --- |
| `CONDITION_SHAPES` | `platform-os` / `failure` | 未知形状 ⇒ 具名 | 每个形状 MUST ≥1 步在用 |
| `RUNNER_OS_VALUES` | `Linux` / `Windows` / `macOS` | 未知取值 ⇒ 具名 | **不要求**每值有实例 |
| `RUNNER_LABEL_PREFIXES` | `ubuntu-` / `windows-` / `macos-` | 不可分类的标签 ⇒ 具名 | 只要求**矩阵里出现过的**标签可分类 |

差异是刻意的：`CONDITION_SHAPES` 是本轮的**规则表**（与 ADR 0077 的 `TIER_PAYMENTS` 同形：
没有实例的规则恒真）；后两张是**平台事实的字典**——要求「Linux 和 macOS 都必须出现在矩阵里」
等于要求矩阵包含所有平台，那是把字典当成了断言。

`PLATFORM_COMMAND_SHAPES` 同属规则表 ⇒ 每个形状 MUST ≥1 实例（实测两者由同一条 `sudo apt-get …`
同时实例化）。

### 决策 8：缺输入按其**需要**具名，MUST NOT 静默跳过

`{host, workflowText, registered}`：缺 `host` 或 `workflowText` ⇒ 具名；解析抛 ⇒ 转成一条具名
错误（MUST NOT 返回空数组——「读不懂」≠「没问题」）；**空注册表**或**一个在册步都没匹配上**
⇒ 具名（`judged === 0` ⇒ 每条规则都无对象可判）。

**一处刻意的非对称**：一个在册步在 workflow 里**不存在**时，本判据**沉默**——那是 W95 的
「a registered step that vanished」判定域。两条判据的判定域由此不重叠。

## 后果

- **零代码改动**：`.github/workflows/foundation.yml` **一个字符都不动**；`WORKFLOW_STEPS` 的既有
  **21 条**一个字符都不动；W95 的 `{identity, condition, run}` 语义不变。零新依赖。
- 新增 `scripts/contracts/step-config.mjs`（判据）、`tests/contracts/step-config.test.mjs`（15 用例）；
  `workflow-steps.mjs` 拆出 `workflowJobsOf` 并给 `workflowStepsOf` 加两个字段。
  Node 套件 **10 → 11**（portable 面 214 → **229** 用例），站点 **12 → 13**。
- 判据 5 组 8 条：可达性、失败语义双向、成因与条件双向、配置面三张闭世界表、平台专属命令、
  两层取值域、非空洞、缺输入具名。
- **注入验证**：V1–V11 全部真改文件、真跑 9 个套件、逐字节还原，`sha256` 逐字一致
  （`foundation.yml` `2f46d5f5…8c1c`）。11 个探针**全部**被至少一条判据注意到。
- **顺带实测到「夹具前提被扰动」这一类假红**：V6/V11 除本判据外还打红了别的套件，
  **根因是那些用例的 `replace` 锚点已被探针改掉**（注入变成 no-op ⇒ 断言前提没了），
  **不是**判据在响——V11 上 `stepConfigErrors` 返回 `[]`（本判据确实沉默）。
  这一类与「P0 行尾翻译」同族：**探针会扰动夹具**，解读跨套件红之前 MUST 先确认前提。
- **如实披露**：
  1. `if:` 的求值是**形状级**的——只认 `runner.os == '<os>'` 与 `failure()`，其余形状
     （`always()`、`github.event_name == …`、`!cancelled()` …）一律**具名拒绝**。拒否方向落在
     **假阳性**（要求人显式归类）。
  2. R5 是形状级，且会扫到 `run` 里的注释文本（一行 `# 需要 sudo` 会让该步被要求平台限定）。
  3. 决策 5 末段的严格方向（同族矩阵下 `platform-toolchain` 判红）。
  4. 「步身份 ↔ 判定目标」的绑定**仍未管**（P-B1）：把一个判定步的 `run` 换成另一条**同样像判定**
     的命令零信号。ADR 0075 只做形状级判定，不绑定「这一步该跑哪个目标」。

> **勘误（W101，2026-10-14）**：上面第 4 条遗留**已关闭**——见
> [ADR 0079](0079-step-target-binding.md)：`scripts/contracts/step-target.mjs` 把「这一步跑的是哪个
> 目标」钉在在册条目上（`target` 与从 `run` 解析出的目标集合**双向对账**）。**原文保留**
> （`docs/adr/` 属**历史记录**，按 §56.2 追加勘误段而不就地改写）。

## 替代方案

- **只做可达性（R1），不做失败语义与成因一致**：P-A3/P-A6 的洞原样留着——一个能被吞掉的判定
  与一条成因未付代价的声明都仍是绿灯。**否决**。
- **把 `matrix` 与 `continue-on-error` 塞进 W95 的 `workflowStepErrors`**：W95 的判定对象是
  **步集合的身份与分类**；执行配置是**步的运行时属性**。合并会让「步不见了」与「步不跑了」
  两类失败共用一条消息，且会让 W95 的既有 37 个用例的前提一并改变。**否决**（改为共享解析器）。
- **用运行时探测决定平台集合**（真跑两个 runner 看谁执行了）：违反**铁律二**——
  「能不能跑」是宿主/平台侧事实，会把瞬态写进判据。**否决**。
- **把支持集写成 `['Linux']`（含 `sudo`）**：在 macOS 矩阵下会误报正确配置（决策 6）。
  **否决**。
- **要求 `RUNNER_OS_VALUES` 每值都有实例**：等于要求矩阵包含所有平台，把字典当断言。**否决**。

## 关联

- 前置：ADR 0073（CI step 步层闭世界）、ADR 0075（被测面包集合）、ADR 0076（npm 脚本执行面）、
  ADR 0077（站点层级声明）；本 ADR 补的是它们都够不着的**执行配置面**。
- 母题第九次：`repositoryFiles` → `readTrackedText` → W94 两张事实表 → W95 `gateSources`
  → W96 门控闭包区域 → W97 包集合 → W98 脚本执行面 → W99 站点层级声明 → **W100 执行配置面**。

## 勘误（W108）

**决策 5 的成因枚举收缩 4 → 3**：`host-subprocess` 从 `CI_ONLY_REASONS` 逐出。理由是**它不可能有
凭据**——它的成因是宿主/机器事实，而宿主瞬态 MUST NOT 成为判定输入（铁律二）⇒ 它是这条规则里一个
**永久无法兑付的空格**（同轮新增的规则：「规则表每格 MUST ≥1 实例」，与本文决策 7 的
「规则表 MUST ≥1 实例」同族）。详见 [ADR 0086](0086-cause-payment.md)。

**决策 5 的规则本身不变**：`platform-toolchain` 仍 SHALL 由**真子集**平台条件兑付，其余成因仍
SHALL NOT 被平台限定。实现侧无需改动——`step-config.mjs` 的分派本来就是按闭集
（`CI_ONLY_REASONS.includes(entry.reason) && restricted`），收缩**自动**正确
（已核对立证：`git diff --stat scripts/contracts/step-config.mjs` 空输出）。

规格里那句「成因是其余**三因**之一却带了平台限定条件」同时改为「成因**不是** `platform-toolchain`
却带了平台限定条件」——**计数是易漂移的，MUST NOT 进活文档**。本文原文保留在上面，不改写。
