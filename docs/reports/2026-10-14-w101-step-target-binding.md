# W101 轮次报告：在册判定步的**目标绑定**必须由 `run` 解析兑付

- 日期：2026-10-14
- 变更：`openspec/changes/2026-10-14-w101-step-target-binding/`
- ADR：[0079](../adr/0079-step-target-binding.md)
- 新增模块：`scripts/contracts/step-target.mjs`
- 新增测试：`tests/contracts/step-target.test.mjs`（16 个用例）

## 一、目标

母题「**声明 MUST 付代价**」的**第十次**换对象。前九次补的全是**集合层**：
`repositoryFiles`（W75）→ `readTrackedText`（W86）→ W94 两张事实表 → W95 `gateSources` →
W96 门控闭包区域 → W97 包集合 → W98 npm 脚本执行面 → W99 站点层级声明 → W100 执行配置面。
本轮补的是**一条声明与它实际跑的对象的配对**——而**集合层对「配对」是瞎的**。

## 二、缝在哪

先说**结论**：W100 §八 1 登记的遗留（P-B1「步身份 ↔ 判定目标」的绑定仍未管，**独立的一层**）
**在本轮被确认为真**，并且比登记时更宽——它不是「一个判断目标没被绑」，而是
**一整个「这一步跑的是什么」的面没人读**。

### 2.1 探针（第一轮：直接改被审对象）

探针逐条真改 `.github/workflows/foundation.yml`、真跑 9 个 portable 套件、逐字节还原
（`read_bytes`／`write_bytes` + 断言 `\r` 计数不变）、`sha256` 逐字一致。基线
`229 passed / 0 failed`（registry 52 / schema 30 / platform 22 / workflow 13 / xtask-commands 17 /
gate-coverage 24 / npm-scripts 34 / tier-payment 22 / step-config 15）。

| 探针 | 改动 | 9 套件失败向量 | 读它的人 |
| --- | --- | --- | --- |
| C1 | 步名 `Scheduling and shared budget ledger` → `Scheduling and shared ledger`（**对照组**） | `[0,0,0,4,0,1,0,0,1]` | W95（+ 两处**前提**被扰动） |
| V1 | `--suite kernel` → `--suite hosts` | `[0,0,0,1,0,0,0,0,0]` | **无**（见下） |
| V2 | `check --profile full` → `check --registry` | `[0,0,0,0,0,1,0,0,0]` | W97 的**越界**断言 |
| V3 | `cargo xtask schema --check` → `cargo xtask schema` | `[0,0,0,0,0,1,0,0,0]` | 同上 |
| V4 | `pnpm spec:validate` → `pnpm check:frontend` | `[0,0,0,2,0,0,5,0,1]` | W95 + W98（+ 前提） |
| V5 | 删掉 CI-only 契约步的 `descriptor.test.mjs` | `[1,0,0,0,0,0,0,4,1]` | W99（+ 前提） |
| V6 | 两处目标互换 | `[0,0,0,1,0,0,0,0,0]` | **无**（夹具） |
| V7 | openspec 步换目标 | `[0,0,0,2,0,0,0,0,1]` | W95（身份＝`run` 首行） |

### 2.2 ⚠️ 第一轮的红**大多不是判据在响**（本轮最大的方法论教训）

逐条读 `not ok` 与它的 assertion message 之后，V1／V6 的红、以及 V5／V2／V3 的部分红，
根因是**夹具锚点被扰动**：

```js
// tests/contracts/workflow.test.mjs:74 「a registered step that vanished is named」
const injected = workflowText.replace(
  '      - name: Kernel registration and binding\n        run: cargo xtask test --suite kernel\n',
  '',
);
```

这条用例的锚点是 **`name` + `run` 一整段**。探针一改那条命令，`.replace()` 就**匹配不到**，
注入变成 no-op ⇒ 用例因**前提没了**而红——**看起来像「判据发现了目标被改」**。
`gate-coverage.test.mjs`（`:232`、`:247`）与 `step-config.test.mjs`（`:421`）有同形的锚点。
V4 的红则来自「身份被顺带改掉」：无 `name` 的步（`- run: pnpm spec:validate`）其
`identity` **就是** `run` 首行，改它的 `run` 会**同时**触发 W95 的「未在册 + 已消失」两条。

### 2.3 第二轮：**避开夹具锚点**的干净探针

只改**没有任何夹具锚定**的那些步（`hosts` / `session` / `scheduler` / `provider` / `knowledge`），
基线 `229 / 0`：

| 探针 | 改动 | 9 套件失败向量 | 结论 |
| --- | --- | --- | --- |
| W0 | 基线 | `[0,0,0,0,0,0,0,0,0]` | — |
| W1 | `Plugin hosts and IPC` 与 `Session leases…` 的 `--suite` **互换** | `[0,…]` | **零信号** |
| W2 | `Scheduling and shared budget ledger` 改跑 `--suite provider` | `[0,…]` | **零信号** |
| W3 | `Knowledge algorithms…` 改跑 `--suite storage` | `[0,…]` | **零信号** |
| W4 | `API Provider transport…` 改跑 `cargo xtask check --secrets` | `[0,…]` | **零信号** |
| W5 | 门控步 `check --profile full` → `check --profile portable` | `gate-coverage=1` | W97 **越界**断言 |
| W6 | `cargo xtask schema --check` → `cargo xtask schema` | `gate-coverage=1` | 同上 |

**W2 最能说明问题**：`scheduler` 那一步不再跑 `scheduler` suite——该判定面**从 CI 静默消失**，
同时 `provider` 被跑两遍；而「21 个步都在册、身份都在、`kind` 都在闭集里、每个 suite 的包集
都被门控覆盖」——**每一格都还是绿的**。W4 更进一步：把一个判定步**整条换掉**（换成另一条
同样合法的判定命令）也**零信号**。

### 2.4 W5/W6 的红是**偶然**，而且那条断言本身越界

```js
// tests/contracts/gate-coverage.test.mjs:439（W101 前的原文）
assert.ok(
  commands.includes('check --profile full') && commands.includes('schema --check'),
  'the gate commands must be recognised after normalisation',
);
```

它钉的是**两个具体串**。这既**假阳性**（把 `full` 合法改成别的 profile 就红），
又**假阴性**（W4 那种同形状替换它看不见）；而且它**违背 W97 自己的边界声明**——
W97 §八 2 明写「**不验签名的取值**」。按对象性质分：**W97 管形状，W101 管取值绑定**。
⇒ 本轮**就地订正**（`tests/` 是**活文档**；`reports/`／`adr/`／`archive/` 才追加勘误段）。

### 2.5 全仓扫描：这句「仍未管」的其他承载物

「步身份 ↔ 判定目标」的绑定仍未管**这句不实陈述**出现在五处，按**载体**分头处置：

| 承载物 | 性质 | 处置 |
| --- | --- | --- |
| `AGENTS.md`（W100 段的如实披露） | **活文档** | **就地订正**（1:1 行替换，指针指向本段） |
| `docs/development-plan.md`（W100 段明确不做） | **活文档** | **就地订正**（同行内改写，行数不变） |
| `docs/adr/0078-step-configuration.md` | 历史记录（已发布） | **追加勘误段** |
| `docs/reports/2026-10-14-w100-step-configuration.md` | 历史记录（已发布） | **追加勘误段** |
| `openspec/changes/archive/2026-10-14-w100-step-configuration/design.md` | 历史记录（已发布） | **追加勘误段** |

（同形不同义：`adr/0078:34`／`report:40`／`archive/design:37` 的表格行描述的是**当时那次探针**
「形状级判定放行」——那是**事实记录**，不是不实陈述，**不订正**。）

## 三、判据设计

新增 `scripts/contracts/step-target.mjs`。三条判据 + 两条纪律：

| 编号 | 判据 | 守卫的探针 |
| --- | --- | --- |
| R1 | 每个 `local`／`ci-only` 条目 MUST 声明 `target`；解析出的目标集合 MUST 与声明**双向相等** | W1／W2／W3／W4 |
| R2 | `infra`／`diagnostic` MUST NOT 声明 `target`，**且**其 `run` MUST 解析出**零个**目标 | V6 |
| R3 | 声明的 `shape` MUST ∈ 闭集、值 MUST 非空 | V10／V9 |
| — | 缺输入 / 空注册表 / 不可解析 ⇒ **具名失败**，MUST NOT 静默降级 | — |

**八个设计决定**（详见 [ADR 0079](../adr/0079-step-target-binding.md) 与变更 `design.md`）：

1. `target` 住**条目自己身上**，MUST NOT 另开一张表（一个 step 是一个对象）。
2. 钉**取值**与 W96「签名不含值」不冲突——W96 管**入口形状**，W101 管**入口实例**。
3. 形状闭集 `TARGET_SHAPES = ['xtask', 'node-test', 'pnpm']`；xtask 那一支**复用** W97 的
   `ciXtaskCommands`（剥壳规则抽成共享的 `stripShellSyntax`，行为一字不变）。
4. **解析前先按 shell 段界拆段**（`&&`／`||`／`;`／`|`／换行）——**假阴性方向必须堵**。
5. `infra`／`diagnostic` **双向**；这比 W95 那条同目的检查**严格**（整段解析 vs 首行 + 5 个正则）。
6. 对账用**集合双向相等**；判定单元是 `(shape, value)` 而非裸值（W91：同名 ≠ 同一个）。
7. 判分由在册条目驱动，**不重复报** W95 的判决（⇒ 对改步名**沉默**）。
8. W97 的越界断言**就地收回形状级**（见 §2.4）。

**决定 4 是设计过程中由测试逼出来的一处修正**：`ciXtaskCommands` 把 `&&…` 整段剪掉，
于是 `… --suite kernel && … check --secrets` 只会解析出**第一条**目标——第二条判定
**静默逃出**。这是**假阴性**方向，按「假阳性与假阴性不对称 ⇒ 永远选安全方向」（W92）必须堵。

## 四、实现

| 文件 | 变化 |
| --- | --- |
| `scripts/contracts/step-target.mjs` | **新增**（`TARGET_SHAPES`／`targetsOf`／`targetKey`／`stepTargetErrors`） |
| `scripts/contracts/workflow-steps.mjs` | 13 个判定条目**增补** `target`；`identity`／`kind`／`localEvidence`／`reason` **一字不动** |
| `scripts/contracts/gate-coverage.mjs` | 抽出 `stripShellSyntax`（**行为一字不变**；`ciXtaskCommands` 改为调用它） |
| `scripts/contracts/test-reachability.mjs` | 新增站点 `xtask-engineering-step-target`（13 → 14）；新增导出 `declaredMarkerPaths()` |
| `tests/contracts/step-target.test.mjs` | **新增**，16 个用例 |
| `tests/contracts/gate-coverage.test.mjs` | 越界断言**就地订正**为形状级 |
| `tests/contracts/tier-payment.test.mjs` / `step-config.test.mjs` | 前提构造改为 `declaredMarkerPaths()`（见 §4.1） |
| `tests/registry.test.mjs` | 逐字站点清单 13 → 14 |
| `xtask/src/lib.rs` | **+8 行**（1 行登记 + 7 行注释）；Rust **零行为改动** |
| `.github/workflows/foundation.yml` | **一个字符未动**（被审对象） |

### 4.1 本轮附带修掉的一个**同族缺陷**：前提构造跟着「轮次」走

新增一个站点，一次让**三条既有用例**变红（`registry`／`tier-payment`／`step-config` 的
可达性正向前提）。根因不是 W101 引入的新问题，而是 W99／W100 把前提写成：

```js
const withNew = new Set([...tracked, 'tests/contracts/<自己那一轮的文件>.test.mjs']);
```

——**前提跟着轮次走，而不是跟着站点表走**。每新增一个站点，就会让**上一轮**用例手写构造的
前提失效。修法：导出 `declaredMarkerPaths()`（**遍历站点表自己的键**、取无通配的 marker 并按
`base` 还原），前提构造改用它 ⇒ 再新增站点不会重演。
这与 W101 的第 8 条纪律同源——「同族手写清单 ⇒ 判据 MUST 遍历对象自己的键、覆盖全部在册对象」。

## 五、非空洞验证

11 条注入 + 1 条对照组，全部**真改文件、真跑 10 个套件、逐字节还原**
（`foundation.yml` `2f46d5f52ce8b9af86c6b8947b0c1f012168f411ac50cc78133a68da1a248c1c`
前后一致；`workflow-steps.mjs` `6758a090584f6bb1…` 前后一致；两文件 `CR=0` 不变）。
与 §2.3 的差别在于：**这是 W101 落地之后**，所以探针由「零信号」变成「具名失败」。

### 5.1 每条注入的**判据裁决**与红点归属

| 注入 | W101 判据裁决 | 变红的套件 |
| --- | --- | --- |
| V1 `hosts`→`session` | **2 条**（runs／declares 两个方向） | 仅 `step-target` |
| V2 `scheduler`→`hosts` | **2 条** | 仅 `step-target` |
| V3 `session`→`check --secrets` | **2 条** | 仅 `step-target` |
| V4 两步目标互换 | **4 条**（两步各两个方向） | 仅 `step-target` |
| V5 `schema --check`→`schema` | **2 条** | 仅 `step-target` |
| V6 判定塞进 `infra` 步 | **1 条**（`is infra but its command yields…`） | `workflow`(W95) / `step-config`(W100) / `step-target` —— **三条断言各不相同** |
| V7 改「身份＝`run` 首行」的步 | **0 条（沉默）** | `workflow`(W95) / `npm-scripts`(W98) / 前提 |
| V8 声明目标值改掉 | **2 条** | 仅 `step-target` |
| V9 声明目标清空 | **1 条**（`names no target`） | 仅 `step-target` |
| V10 目标形状域外 | **1 条**（`not a target shape`） | 仅 `step-target` |
| V11 门控步 `run` 清空 | **1 条**（`yields none`） | `gate-coverage`(W97 形状) + `step-target` |
| C1 改步名（**对照组**） | **0 条（沉默）** | `workflow`(W95) + 两处**前提** |

**边界矩阵**（列 = registry / schema / platform / workflow / xtask-commands / gate-coverage /
npm-scripts / tier-payment / step-config / step-target；即 W84 / — / — / W95 / W96 / W97 / W98 /
W99 / W100 / W101）：

```
V1  0 0 0 0 0 0 0 0 0 2      V7  0 0 0 2 0 0 5 0 1 2
V2  0 0 0 0 0 0 0 0 0 2      V8  0 0 0 0 0 0 0 0 0 2
V3  0 0 0 0 0 0 0 0 0 2      V9  0 0 0 0 0 0 0 0 0 2
V4  0 0 0 0 0 0 0 0 0 2      V10 0 0 0 0 0 0 0 0 0 2
V5  0 0 0 0 0 0 0 0 0 2      V11 0 0 0 0 0 1 0 0 0 3
V6  0 0 0 2 0 0 0 0 2 2      C1  0 0 0 2 0 0 0 0 1 2
```

**V1–V5、V8–V10 只有 W101 一个判据在响**（其余九列全 0）——这是「本轮补的确实是一层新对象」
的直接证据。V6／V11 的交叉红**不是重复定义**，已逐条定性为**不同断言**：

- V6：W95 报 `looks like a judgement`（**首行 + 5 个正则**的形状级）、W100 报平台专属命令形状的
  **非空洞**（`sudo`／`apt-get` 的实例被拿掉）、W101 报 `is infra but its command yields the
  target(s) …`（**整段解析**）；
- V11：W97 报 `the gate-command shape must be recognised`（**形状级**，订正后仍有权重）、
  W101 报 `declares … but its command yields none`。

### 5.2 对照组 C1：**改步名时本判据沉默**

`stepTargetErrors` 在 C1 下返回 `[]`——因为身份不是 W101 的判定对象（那是 W95 的）。
C1 的两处 `step-target` 红**全部是前提**：

1. `every shipped judging step pays for the target it runs` ——本轮**新加了一条具名前提断言**
   （「在册与解析出的身份集合 MUST 一致；那是 W95 的事，本判据不重复报」），它先红；
2. `the judge is orthogonal to W95…` ——该用例的前提是「出厂状态 W95 一致」。

⇒ **这正是「两层分开测」的实证**：身份层（W95）与目标层（W101）各有各的对象，
把「改步名」当成「改目标」会得出错误的结论。

### 5.3 真实仓库的前置条件

`every shipped judging step pays for the target it runs` 逐条比对
「该步**解析出的**目标 == 它**声明的**目标」（`xtask` 10 / `node-test` 1 / `pnpm` 2 共 13 条声明，
另 7 `infra` + 1 `diagnostic` 不带目标），并用**具名前提断言**保证身份集合一致。

## 六、门控与 validate

### 六.1 全量门控 `cargo xtask check --profile full`

- 环境：`source /c/Users/32791/.nai-msvc-env.sh` + `CARGO_TARGET_DIR=E:/nai-build-target CARGO_INCREMENTAL=0`。
- **第一次运行 ⇒ `EXIT=1`，但失败点不是本轮的代码**：末段报
  `error: linking with link.exe failed: exit code: 1104` /
  `LINK : fatal error LNK1104: 无法打开文件"E:\nai-build-target\debug\deps\nai_xtask-01d6cdd49aa03c60.exe"`，
  冒泡为 `xtask: cargo test --locked --package nai-xtask --lib failed: exit code: 101`。
  此前全部目标（`cargo fmt --check`、`clippy -D warnings`、`cargo test --workspace` 与全部 `Doc-tests`、
  10 个 node 契约套件、`node scripts/frontend-gate.mjs`）**都已通过**。
- **按既定判据定性**：单独复跑该失败目标
  `cargo test --locked --package nai-xtask --lib` ⇒ `EXIT=0`，`test result: ok. 52 passed; 0 failed`。
  逐目标能过 ⇒ **非代码缺陷**（宿主 `LNK1104` 间歇）。
- **处置**：限并发 `CARGO_BUILD_JOBS=1` 重跑整个 `--profile full` ⇒ **`EXIT=0`**，末行
  `full: engineering foundation checks incl. the frontend gate passed; device/SDK/GPU/install certification not performed`。
  **代码一字未改**。
- **套件计数**（限并发那次）：registry 52 / schema 30 / platform 22 / workflow 13 / xtask-commands 17 /
  gate-coverage 24 / npm-scripts 34 / tier-payment 22 / step-config 15 / **step-target 16（本轮新增）**
  = **245 passed / 0 failed**；另 `cargo test --package nai-xtask --lib` 52 passed，
  `node scripts/frontend-gate.mjs` 5 passed。
  与 W100 基线（229）之差 **+16**，恰为本轮新增套件的用例数。

### 六.2 两条 validate

- `openspec archive 2026-10-14-w101-step-target-binding --yes` ⇒ 归档为
  `openspec/changes/archive/2026-10-14-w101-step-target-binding`；规格差量
  `engineering-foundation: + 1 added`。
- `validate --all --strict` ⇒ **43 passed, 0 failed (43 items)**。
- `validate --all --strict --archived` ⇒ **99 passed, 0 failed (99 items)**（含本轮条目
  `✓ change/2026-10-14-w101-step-target-binding`）。

## 七、CI

推送 `759a26c` 后做**单次低频观测**（唯一查询参数破坏代理缓存 + `Cache-Control: no-cache` +
回读 `head_sha` 与本地 `HEAD` 比对）：

- run **#135** `Engineering foundation`，`id=38505704125`；
  `head_sha=759a26c06d225a51c553e18c52062cdae2d84203` == 本地 `HEAD`（逐字一致）；
  `status=completed`，`conclusion=success`；`run_started_at=2026-10-14T03:20:04Z` →
  `updated_at=2026-10-14T03:32:04Z`。
- 两个 job 均 `completed / success`：
  - `foundation (windows-latest)`：03:20:08Z → 03:32:03Z；
  - `foundation (ubuntu-24.04)`：03:20:08Z → 03:25:58Z。
- 两条腿的各判定步（`Storage and credential boundary` / `Kernel registration and binding` /
  `Plugin hosts and IPC` / `Session leases and independent cancellation` /
  `Scheduling and shared budget ledger` / `API Provider transport and retry` /
  `Knowledge algorithms …` / `Validate schemas and API contract`）**逐条 step 皆 success**，
  即本轮 `target` 绑定的那 13 条在册判定步在 CI 上**确实跑的是它们各自声明的目标**。
  ⚠️ `Install Linux C++ protobuf runtime` 与 `Full three-language contract suite` 在 Windows 腿
  为 `skipped`（平台专属，**预期内**，非本轮引入）；失败摘要 annotation 步同样 `skipped`
  （因为**没有失败**）。
- 观测共 12 次 API 请求（1 次列表 + 10 次 run + 1 次 jobs），间隔 ≥75s，**未轮询**；
  `resources.core` 未打满。

## 八、遗留

1. **`run` 里的 shell 注释不剥**：`# cargo xtask …` 会被当成一个目标（W97 的既有边界，本轮沿用）。
   假阳性方向，安全。
2. **不实现 shell 语义**：变量展开（`${{ matrix.os }}`）、条件分支、函数体一概不解析
   ⇒ `if`／`for` 包裹的命令会被当成**无条件目标**。假阳性方向，安全。
3. **每个形状的实例数不钉**（只钉闭集非空）：一次合法的目标收敛（例如把 `node-test` 步改成
   `xtask` 步）不该让判据变红（W92 的教训）。当前实测 `xtask` 10 / `node-test` 1 / `pnpm` 2。
4. **不判目标在矩阵各腿上的等价性**：那是 W100 的执行配置面。
5. **`pnpm` 形状只认 `pnpm <…>`，不认 `npx`／`node_modules/.bin/…`**：当前仓库没有这种写法；
   出现时会被「解析出零个目标」具名，而不是静默放过。
6. **W100 §八 1 已由本轮关闭**（§2.5 已按载体分头订正/勘误）。
7. W90 §八 1/3/4/5、W91 跨语言键级消费、W92 基线收录、W93 锚点唯一性、W94 `needs_external` 审计、
   W95 `ci-only` 成因、W96 五项、W97 六项、W98 §八 1/2/7、W99 §八 1/2/3、W100 §八 2/3/4 **均未动**。

## 九、如实披露

1. **§2.2 的探针陷阱是本轮最大的方法论收获**：第一轮 8 条探针里有 **6 条的「红」不是判据在响**，
   而是夹具锚点被扰动或身份被顺带改掉。**跨套件红 MUST 逐条读 `not ok` 与它的 assertion
   message**，否则会把「前提没了」当成「判据发现了问题」——**这正是「短路＝把未检查伪装成已通过」
   的镜像**：把「前提坏了」当成「检查过了」。
2. **`target` 的拒否是形状级的**：`pnpm` 形状只排除 `install`（`install --frozen-lockfile` 等
   变体由前缀匹配覆盖，但 `pnpm dlx`／`pnpm run build` 之类会被当成目标 ⇒ 假阳性，安全）。
3. **V7 型注入本判据沉默是有意的**：身份被改掉时 W95 会报两条，W101 再报一遍会让边界变糊。
   代价是：若有人**只**改一条 `run` 首行而**同时**改掉在册身份，本判据不会额外提供信息——
   但那种改动已经被 W95 拦下。
4. **`declaredMarkerPaths()` 是测试前提的工具，不是判据**：它让「站点表已被完全兑付」这个前提
   由站点表自己构造。它**不能**替代真实检查——「一个站点覆盖不到任何已跟踪文件 = 谎言」
   仍由 `tests/registry.test.mjs` 的 F0 用**真实** `.git/index` 验。
5. **本轮动了两个既有测试文件的断言/前提**（`gate-coverage.test.mjs` 的越界断言、
   `tier-payment.test.mjs` 与 `step-config.test.mjs` 的前提构造）。
   前者是**订正不实/越界的声称**，后者是**修前提而非改断言**（铁律一）——
   两条都不是「为了变绿而放松判据」：订正后 W97 的**形状**声称仍有权重（V11 实测仍红），
   前提改法**只会让更多站点被兑付**（`declaredMarkerPaths` 遍历整张表，MUST NOT 只补一个）。
