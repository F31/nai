# w100 设计

## 一、问题形状：**一个面**没人读，而不是一个字段没人读

W95 的解析器只产出三个字段，`matrix` 与 `continue-on-error` **根本不进入判定域**；
而 `condition` 只被 `diagnostic` 分支读了一次。于是「这个步会不会跑、在哪儿跑、失败了算不算数」
这三个问题**在判据里没有答案**：

```
在册步（21 条）            kind         执行平台集合        失败会否被吞        谁读过
  actions/checkout@v4 ... infra ........ 全部 ............. — ................ 无人
  Install Linux C++ …… ... infra ........ Linux（1/2）...... — ................ 无人  ← P-A4 改的是它
  Complete engineering … . local ........ 全部 ............. 否 ................ 无人  ← P-A3 改的是它
  Storage and credential . local ........ 全部 ............. 否 ................ 无人
  …（另有 7 条 local）                                                        ← P-A6 改的是其一
  Full three-language … .. ci-only ...... Linux（1/2）...... 否 ................ 无人  ← P-A1 改的是它
  Validate schemas …… .... ci-only ...... 全部 ............. 否 ................ 无人
  Publish the gate …… .... diagnostic ... 全部 ............. **是**（有意）...... 无人（W95 只读 if）
```

⇒ 与 W75 / W86 / W94 / W95 / W96 / W97 / W98 / W99 同族：**声明不付代价**。
区别在于，前面几轮各补了**一个字段**或**一个集合**的兑付；本轮补的是一个**二维面**
（`matrix` 的轴 × 每步的 `if:`）**加**一个**开关**（`continue-on-error`）。

## 二、证据（探针，改真文件 + 真跑 + 逐字节还原 + `sha256` 逐字一致）

`.baseline`：8 个 portable 套件合跑 **214 passed / 0 failed**；
`foundation.yml` 的 `sha256 = 2f46d5f52ce8b9af86c6b8947b0c1f012168f411ac50cc78133a68da1a248c1c`。

| 探针 | 改动 | 结果 |
| --- | --- | --- |
| **P-A1** | 三语言契约套件步的 `if: runner.os == 'Linux'` → `'FreeBSD'` | `[0,0,0,0,0,0,0,0]` ⇒ **零信号** |
| **P-A2** | 矩阵 `os: [ubuntu-24.04, windows-latest]` → `[windows-latest]` | `[0,0,0,0,0,0,0,0]` ⇒ **零信号** |
| **P-A3** | `local` 门控步加 `continue-on-error: true` | `[0,0,0,0,0,0,0,0]` ⇒ **零信号** |
| **P-A4** | 删掉 Linux C++ 安装步的 `if: runner.os == 'Linux'` | `[0,0,0,0,0,0,0,0]` ⇒ **零信号** |
| **P-A6** | `local` provider 步加 `continue-on-error: true` | `[0,0,0,0,0,0,0,0]` ⇒ **零信号** |
| **P-B1** | 判定步 `run` 换 `check --registry`（标签不变） | `[0,0,0,0,0,0,0,0]` ⇒ 形状级判定放行（§六 4） |
| **P-A5** | 删掉诊断步的 `if: failure()` | `workflow` 套件 **2 条红** ⇒ W95 已兑付（**对照组**） |
| 还原 | 逐字节还原（`read_bytes`/`write_bytes` + 断言 `\r` 计数不变） | `RESTORE OK`，`sha256` 逐字一致 |

P-A5 是对照组：它证明「零信号」不是因为判据整体失效，而是因为 **P-A1…P-A4/P-A6/P-B1 落在一个
从未被判据覆盖的面上**。

### ⚠️ 探针方法论的两条教训（本轮实测踩到，写下来供后续轮次复用）

1. **Python `Path.write_text` 在 Windows 上把 `\n` 翻成 `\r\n`**，会破坏测试夹具里所有含 `\n`
   的字面锚点 ⇒ 探针**自己制造红/破坏夹具**。首轮实测 P-A1/A2/B1 得到 `[0,0,0,4,0,1,0,0]`，
   排查后确认是行尾翻译，不是信号。**修法 = `read_bytes`/`write_bytes` 逐字节写入 +
   断言「`\r` 计数与原文一致」**。重跑后三项全 `0`。
2. **某些步名被测试当作夹具锚点**（实测 `Kernel registration and binding` 在
   `workflow.test.mjs` 里出现 11 次、`Install Linux C++ protobuf runtime` 1 次）。
   对它们注入会**扰动夹具** ⇒ 假红。P-A6 原用前者得到 1 条假红，改用零命中的
   `API Provider transport and retry (mock server; no paid calls)` 后为 `0`。
   **修法 = 探针选「不被任何测试当锚点」的步**（先统计命中次数再选）。

## 三、设计决定

### 决定 1：解析器**只留一份**，但产出**两个视图**

```
workflowJobsOf(text)   → [{id, matrix, runsOn, steps: [{identity, condition, run, continueOnError}]}]
workflowStepsOf(text)  → [{identity, condition, run, matrix, continueOnError, job}]   ← 扁平视图
```

`workflowStepErrors`（W95）继续用扁平视图；`stepConfigErrors`（本轮）用**job 视图**
（它需要 `matrix`，而 `matrix` 是**job 的属性**，不是 step 的）。两个视图来自**同一次 YAML 解析**：
「同一件事 MUST NOT 两处定义」——两份解析器一定会漂移。

`matrix` 在扁平视图里对同 job 的步是**同一个对象引用**，不是各自拷贝：拷贝会让
「同一个矩阵」变成 21 份可分别篡改的副本。

**兼容性**：`{identity, condition, run}` 的语义一字不动，`workflowStepErrors` 的入参签名不变
⇒ W95 的 37 个用例全部不变（实测）。

### 决定 2：可达性由**静态求值**得出，不由「条件非空」得出

```
platformLegsOf(step, job) → Set<os>        // 该步真正会跑的平台集合
  legs = platformsOf(job)                  // matrix.os 的取值，或字面 runs-on
  if condition === ''            → legs
  if condition === platform-os X → legs ∩ {L : family(L) === X}
  if condition === 'failure()'   → legs            // 失败条件不筛平台
```

判定 **MUST 是集合级**的，MUST NOT 是「`if:` 字段存在与否」：
`runner.os == 'macOS'` 是一个**非空条件**，但当前矩阵里没有 macOS ⇒ 该步**永不跑**。
只问「有没有 `if:`」会把这种情形放过去——这正是 P-A2/P-A1 的洞。

### 决定 3：R1 与 R4 **分两层**拒否，各自具名

| 情形 | 谁拦 | 为什么不是另一条 |
| --- | --- | --- |
| `runner.os == 'FreeBSD'` | **R4**（取值域外） | GitHub runner 从不报 `FreeBSD` ⇒ 这是一个**读不懂的取值**，不是「跑了但没跑成」 |
| `runner.os == 'macOS'`（矩阵里没有） | **R1**（集合为空） | 取值合法、条件可读，但它对**当前矩阵**恒假 ⇒ 这是一个**不跑的判定** |
| `matrix.os: []` | **R4**（矩阵为空） | 空矩阵 ⇒ 零个 job ⇒ 连「不跑」都算不上，是**声明本身空洞** |

两条判据的输入不同（R4 看取值域，R1 看矩阵），**合并不了一处**：
合并后错误消息就分不清「你写了个不存在的平台」与「你的矩阵里没有这个平台」。

### 决定 4：R2 是**双向**的——判定不可被吞，诊断不可替换结论

```
kind ∈ {local, ci-only}  → continue-on-error MUST NOT be truthy   // 判定的结论必须能红
kind === 'diagnostic'    → continue-on-error MUST be truthy       // 诊断的问题不得替换原始结论
kind === 'infra'         → 不判（见 §六 3）
```

为什么 `diagnostic` 那一半不是过度设计：诊断步**只在失败时跑**（W95 已用 `if: failure` 钉住），
它的工作是**转译**已有失败。如果转译器本身能失败并让 job 因为**它的**原因变红，
那么「CI 红」这件事的**因果**就被替换了——这正是仓库里那句注释
（`continue-on-error 保证诊断自身的问题不会掩盖或替换原始失败的结论`）想表达的东西。
**注释是散文，判据才是兑付。**

实测：真实仓库上 `diagnostic` 步有 `continue-on-error: true`，`local`/`ci-only` 步一个都没有
⇒ **基线绿**；把任一侧反过来 ⇒ 具名红。

### 决定 5：R3 按**成因表**分派，MUST NOT 写死步名

```
reason === 'platform-toolchain' → legs MUST ⊊ platformsOf(job) 且非空   // 平台限制是它的兑付形式
reason ∈ 其余三因                → legs MUST === platformsOf(job)         // 平台限制与因由矛盾
```

这与「分派 MUST 由类别表定、MUST NOT 写死字段名」同纪律：新增一个 `ci-only` 原因时，
`CI_ONLY_REASONS` 会先红（W95 的闭世界），归类者必须同时回答「它靠什么兑付」。
**「平台限制」不是任何一类 `ci-only` 的通用装饰**——`host-subprocess` 是宿主 CPU 派生子进程的能力
问题，与 runner 的 OS 无关；给它在 Linux 上加 `if:` 只会让 Windows 上**静默不跑**。

反向（`MUST NOT 被平台限定`）在真实仓库上的实测：四个非 `platform-toolchain` 的 `ci-only` 步
全部无 `if:` ⇒ 绿。

### 决定 6：R5 用**命令形状**而非语义，并**按形状声明支持集**

```
{id: 'posix-privilege-escalation', supported: ['Linux','macOS'], pattern: /(?:^|[\s;&|])sudo\s/}
{id: 'debian-package-manager',     supported: ['Linux'],        pattern: /(?:^|[\s;&|])apt-get\s/}
```

两条决定值得写下来：

- **`sudo` 的支持集不是 `['Linux']`**。本机与 GitHub 的 macOS runner 都有 `sudo`，
  只有 Windows runner 没有。把支持集写成 `['Linux']` **在 macOS 矩阵下会假红**
  （假阳性虽安全，但它会误报一个**正确**的配置）。这里改成 `['Linux','macOS']`，
  与 `apt-get` 的 `['Linux']` 分开——**形状的支持集 MUST 按实测的平台事实写**。
- **形状扫 `run` 的整段文本，不是首行**。`Install Linux C++ protobuf runtime` 的首行是
  `sudo apt-get update && sudo apt-get install -y libprotobuf-dev`：`^apt-get` 匹配不到
  （它在 `&&` 之后）。只用首行会让 `debian-package-manager` 这一格**零实例** ⇒
  与「每个形状 MUST 有实例」的非空洞断言冲突。扫整段 + 行首/分隔符锚定，两个形状都被实例化。

**如实披露**：这是**形状级**判定，且 `run` 里的注释文本也会被扫到
（一行 `# 需要 sudo` 会让该步被要求平台限定）⇒ 假阳性方向。安全，记入 §六 5。

### 决定 7：R4 的闭世界**三份表**，且**每格 MUST 有实例**

| 表 | 键/取值 | 域外 | 非空洞 |
| --- | --- | --- | --- |
| `CONDITION_SHAPES` | `platform-os` / `failure` | 未知形状 ⇒ 具名 | 每个形状 MUST ≥1 步在用 |
| `RUNNER_OS_VALUES` | `Linux` / `Windows` / `macOS` | 未知取值 ⇒ 具名 | **不要求**每值有实例（矩阵里有什么就该认什么） |
| `RUNNER_LABEL_PREFIXES` | `ubuntu-` / `windows-` / `macos-` | 不可分类的 runner 标签 ⇒ 具名 | 只要求**矩阵里出现过的**标签可分类 |

三张表的非空洞要求**刻意不同**，理由各写在各行：`CONDITION_SHAPES` 是本轮**自己的规则表**
（与 W99 的 `TIER_PAYMENTS` 同形：没有实例的规则恒真）；而 `RUNNER_OS_VALUES` / 标签前缀表是
**平台事实的字典**——要求「Linux 和 macOS 都必须出现在矩阵里」等于要求矩阵包含所有平台，
那是把字典当成了断言。

### 决定 8：缺输入按其**需要**具名，MUST NOT 静默跳过

入参 `{workflowText, registered}`：
- `workflowText` 不是字符串 ⇒ 具名报错；
- 解析抛错 ⇒ 转成一条具名错误，MUST NOT 返回空错误数组（「读不懂」≠「没问题」）;
- `registered` 缺省取 `WORKFLOW_STEPS`，但显式给空数组时 **R1/R2/R3/R5 无对象可判**
  ⇒ 由非空洞断言（「至少一个在册步」）拦住，MUST NOT 让空注册表安静通过。

## 四、边界（与相邻判据互不顶替；实测数字见报告 §五）

| 判据 | 管什么 | 本轮的**独有**覆盖 |
| --- | --- | --- |
| W95 `workflowStepErrors` | 步**在不在**、`kind`/`reason`/`localEvidence`、身份唯一 | 它读 `condition` **只**为判定 `diagnostic`（`includes('failure')`）；**不读** `matrix`/`continue-on-error`，也不问「条件筛掉后还剩几个平台」 |
| W97 `coverageErrors` | 门控跑哪些包、suite 跑哪些包、CI 调用哪些命令 | 命令**面**；本轮管执行**配置**面 |
| W98 `scriptErrors` | 脚本「谁调用它」 | 无交叠（不同对象：npm 脚本 vs workflow 配置） |
| W99 `tierErrors` | 站点声明的**层级**由谁兑付 | 它读 workflow 的 `run` 文本判「有没有 CI step 跑它」；**不读**该 step 会不会跑、在哪跑、是否被吞 |
| W84 `reachabilityErrors` | 测试文件**在不在**覆盖集里 | 无交叠 |

**与 W95 的一处刻意重叠必须说清**：两条判据都读 `condition`。W95 读它是为了
「`diagnostic` 步必须带失败条件」（**该步的角色**）；本轮读它是为了「条件筛掉后平台集合非空」
（**该步的可达性**）。**判定域不同、失败消息不同**，不是重复定义。

## 五、非空洞

- 三份规则表**非空且逐格有实例**被断言（见决定 7）。
- 五种 kind 里，`local`/`ci-only`/`diagnostic` 三格**都**被 R2 触碰到（两种方向各 ≥1 实例）。
- R3 的四类成因里，`platform-toolchain` 走正向、其余三因走反向，**两个方向在真实仓库上都被实例化**
  （正向 1 条、反向 4 条）。
- R5 的两个形状**都**在真实仓库上有实例（同一条步的两个 token）。
- 每条判据都有**注入用例**（合成世界 + 真实文件注入），且每条注入 MUST 具名失败；
  反向（正配置时不红）也有用例。

## 六、明确不做 / 如实披露

1. **不验测试能跑通**（要执行；且**铁律二**：宿主瞬态 MUST NOT 改变结论）。
2. **不实现 GitHub 的 `if:` 求值语义**：只做闭形状分类。`always()`、`success()`、
   `github.event_name == …`、`!cancelled()` 等一律**具名拒绝** ⇒ 未来任何新形状都要求人显式归类。
3. **不判 `infra` 步的 `continue-on-error`**：环境准备失败会让所有下游判定步失败，
   是**自曝**而非静默。`infra` 只被 R5 读（平台专属命令）。
4. **不判「步身份 ↔ 判定目标」的绑定**：P-B1 显示把一个判定步的 `run` 换成另一条**同样像判定**
   的命令（`cargo xtask check --registry`）零信号。W97 只做**形状级**判定（「是不是一条判定命令」），
   **不**绑定「这一步该跑哪个目标」。这是**独立的**一层，记入 §八 遗留。
5. **R5 是形状级且会扫到 `run` 里的注释文本**（决定 6 末段）：假阳性方向。
6. **不改任何被审对象**（除 `lib.rs` 加一行登记、`test-reachability.mjs` 加一条站点）：
   `foundation.yml` 与 `WORKFLOW_STEPS` 的 21 条**一字不动**。
7. W90/W91/W92/W93/W94/W95/W96/W97/W98/W99 的登记遗留**均未动**。
