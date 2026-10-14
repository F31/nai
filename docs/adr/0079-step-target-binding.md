# ADR 0079：在册判定步的**目标绑定**必须由 `run` 解析兑付

- 状态：已接受
- 日期：2026-10-14
- 轮次：W101
- 关联：[ADR 0073](0073-workflow-step-closure.md)（步层闭世界）、[ADR 0075](0075-gate-coverage.md)（包集合与命令面）、
  [ADR 0077](0077-tier-payment.md)（站点层级兑付）、[ADR 0078](0078-step-configuration.md)（执行配置面）

## 背景

W95 给 `.github/workflows/foundation.yml` 的 step 集补了**步层闭世界**：步的**身份**由解析得出、
与在册清单双向一致、`kind` 落在闭集里、`local` 要引出真的门控落点、`ci-only` 要给出成因、
`diagnostic` 必须由 `if: failure…` 认出。W96 管住了 `cargo xtask` 的命令面，W97 管住了包集合与
「CI 调用哪些命令」，W99 管住了站点的层级兑付，W100 管住了执行配置面（跑不跑／在哪儿跑／失败算不算数）。

这些判据全部是**集合层**的。而**集合层对「配对」是瞎的**：在册条目只声明 `{host, identity, kind}`，
其中 **`identity` 是名字，不是目标**。于是：

```yaml
- name: API Provider transport and retry (mock server; no paid calls)
  run: cargo xtask test --suite provider        # ← 这个名字与这条命令之间**没有任何断言**
```

一条声明与它实际跑的**对象**之间的配对，从来没有被任何判据读过。

### 实测（探针，改真文件、真跑 9 个 portable 套件、逐字节还原、`sha256` 逐字一致）

基线 `229 passed / 0 failed`。

| 探针 | 改动（真改 `.github/workflows/foundation.yml`） | 失败向量（9 套件） |
| --- | --- | --- |
| W1 | `Plugin hosts and IPC` 与 `Session leases…` 的 `--suite` **互换** | `[0,…]` **零信号** |
| W2 | `Scheduling and shared budget ledger` 改跑 `--suite provider` | `[0,…]` **零信号** |
| W3 | `Knowledge algorithms…` 改跑 `--suite storage` | `[0,…]` **零信号** |
| W4 | `API Provider transport…` 整条改跑 `cargo xtask check --secrets` | `[0,…]` **零信号** |
| W5 | 门控步 `check --profile full` → `check --profile portable` | `gate-coverage=1`（**偶然**） |
| W6 | `cargo xtask schema --check` → `cargo xtask schema` | `gate-coverage=1`（**偶然**） |

W5/W6 的红不是目标绑定在响，而是 W97 的一条**越界断言**：

```js
assert.ok(commands.includes('check --profile full') && commands.includes('schema --check'), …)
```

它钉的是**两个具体串**——与 W97 **自己的边界声明**（「不验签名的取值」，W97 §八 2）相矛盾。
它同时是**假阳性源**（合法改 profile 即红）与**假阴性源**（W4 那种同形状替换它看不见）。

⇒ 母题「**声明 MUST 付代价**」的第十次出现。前九次分别补 `repositoryFiles`、`readTrackedText`、
W94 两张事实表、W95 `gateSources`、W96 门控闭包区域、W97 包集合、W98 npm 脚本执行面、
W99 站点层级声明、W100 执行配置面；**本轮补的是一条「声明 ↔ 它跑的对象」的配对**。

## 决策

### 决策 1：`target` 住**在册条目自己身上**，MUST NOT 另开一张表

一个 step 是**一个对象**：身份、类别、目标、凭据都是它的属性。`WORKFLOW_STEPS` 已经是
「步身份 ↔ 类别」的唯一在册处，闭世界（未在册／已消失）也长在它上面。再开一张 `step → target`
的表就是**同一件事的第二处定义**——两张表会各自漏掉对方的新增。

### 决策 2：`target` 钉**取值**，这与 W96「签名不含值」不冲突

W96 的 `dispatchSignatures` 描述「**入口的形状**」：`check --profile <P>` 是一条签名，`<P>` 不进那张表，
因为它管的是「命令面有哪些入口」。W101 问的是**另一个问题**：「**这一步实际调用的是哪一个入口实例**」
——它的答案里**必须**含 `full`，否则无法把「跑 `full`」与「跑 `portable`」分开。
同一条命令在两个不同的判定域里被问了两个不同的问题，两个答案都必要。

### 决策 3：形状闭集 `TARGET_SHAPES = ['xtask', 'node-test', 'pnpm']`；xtask 那一支**复用** W97 的解析

闭集让「未知形状」具名失败（W90）。三个取值由**实测**得出，各有活实例：`xtask` 10、`node-test` 1、`pnpm` 2。

「一段文本里有哪些 `cargo xtask …` 命令」这件事已经有第一处定义（W97 的 `ciXtaskCommands`，它处理了
`2>&1`、`| tee …`、`&&`、`;`、行续符）。W101 **调用**它，MUST NOT 写第二份剥壳逻辑——第二份实现一定会漂移
（门控步的 `| tee` 就在那条命令里）。

### 决策 4：解析前**先按 shell 段界拆段**——这是**假阴性**方向必须堵的洞

`cargo xtask test --suite kernel && cargo xtask check --secrets` 里**两条**命令都会被 shell 执行，
而「一行取一条」的做法（`stripShellSyntax` 把 `&&…` 整段剪掉）只会看到第一条 ⇒ 第二条判定
**静默逃出**目标绑定。段界 = `&&`、`||`、`;`、`|`、换行；每一段各自抽取，于是每一段各自对账。
W97 的 `ciXtaskCommands` **保留它自己的边界**（本轮不动它一个字符，只把「剥壳」抽成一处共享的
`stripShellSyntax`，行为一字不变）。

### 决策 5：`infra` / `diagnostic` **双向**约束——不声明目标，且**解析出零个**目标

单向（只禁声明）是装饰：真正的后门是「infra 的 `run` 里塞一条判定」。W95 已经有一条这样的检查，
但它**只看首行**、只认 **5 个正则**（`cargo xtask` / `node --test` /
`node --experimental-strip-types` / `pnpm exec` / `pnpm <非 install>`）。W101 从**解析**侧补：整段 `run`
解析出的目标集合 MUST 为空。两者对象不同——W95 问「这条命令像不像判定」，W101 问「它解析出的目标集合
是不是空的」。实测 V6（把 `node --test …` 塞进 `Install Linux C++ protobuf runtime`）让两条**都**红，
但断言不同（W95 报「looks like a judgement」，W101 报「is infra but its command yields the target(s) …」）。

### 决策 6：对账用**集合双向相等**，MUST NOT 用「声明 ⊆ 解析」单向

- 解析出而没声明（W1/W4）⇒ 目标被**悄悄换掉**；
- 声明了而没解析出（`run` 被清空）⇒ 凭据是**空话**。

两个方向都具名。集合元素用 `(shape, value)` 的规范键 —— 判定单元 MUST 是**身份**，MUST NOT 是裸值
（W91：`--suite kernel` 这样的串在别的形状里也可能出现）。

### 决策 7：判分由**在册条目**驱动，不重复报 W95 已报的东西

判分按 `kind` 分派，MUST NOT 写死步名（W88）。未知 `kind`、已消失的步、重复身份一律**跳过**——
那是 W95 的判决，两处定义会让边界变糊。⇒ W101 判据对**改步名**是**沉默**的（实测 C1：
`stepTargetErrors` 返回 `[]`，W95 报 2 条）。

### 决策 8：W97 的那条越界断言**就地收回形状级**

`tests/contracts/gate-coverage.test.mjs` 原先钉 `check --profile full` 与 `schema --check` 两个具体串。
按「同一件事 MUST NOT 两处定义」与「按对象性质分」，把它改为**形状级**
（`/^check --profile \S+$/`、`/^schema(?: --check)?$/`），取值绑定只留 W101 一处。
`tests/` 是**活文档** ⇒ 就地订正（历史记录才追加勘误段）。

### 决策 9：同族用例的**前提构造 MUST 遍历站点表自己的键**

W101 新增一个登记站点，一次让**三条既有用例**变红（`registry` / `tier-payment` / `step-config` 的
可达性正向前提）。根因：W99／W100 各自把前提写成 `new Set([...tracked, '<自己那一轮的文件>'])`
—— **前提跟着轮次走，而不是跟着站点表走**。修法是导出 `declaredMarkerPaths()`（遍历站点表、
取无通配的 marker），前提构造改用它。⇒ 再新增站点不会重演这类失效。

## 后果

- **零产品代码改动**：`.github/workflows/foundation.yml` 一个字符不动；`WORKFLOW_STEPS` 的
  `identity` / `kind` / `localEvidence` / `reason` 一个字符不动（只**增补** `target`）；零新依赖。
- `tests/`（`tests/ui` 除外）与 `apps/desktop/tests` 下的 Node 套件 **11 → 12**，portable 面
  **229 → 245**；登记站点 **13 → 14**；`engineering_checks` 多一行。
- 注入 **V1–V11** 全部产生具名错误并逐字节还原；**边界**（W84／W95／W96／W97／W99／W100）
  里 V1–V5、V8–V10 **只有 W101 一个判据在响**（其余全 0），V6／V11 的交叉红已逐条定性为
  **不同断言**（W95「像不像判定」/ W100「平台命令非空洞」/ W97「形状」）。
- **遗留（如实登记）**：`run` 里的 shell 注释不剥（W97 的既有边界，假阳性方向安全）；
  不实现 shell 语义（变量展开、条件分支）；不判目标在矩阵各腿上的等价性（W100 的对象）；
  每个形状的**实例数**不钉（W92 的教训：一次合法的目标收敛不该让判据变红）。

## 替代方案

1. **把 `target` 写进 `localEvidence.marker`**：marker 是「门控落点里逐字出现的串」，
   它回答「本机也跑吗」，不回答「跑的是哪个目标」；两者在门控步上会指着**同一个串**而掩盖差别。
2. **只钉「命令的集合」而不钉「配对」**（W97 已经这么做）：集合相同、配对错（W1/V4）时完全沉默；
   而配对错会让**名字开始说谎**。集合层对配对是瞎的。
3. **给 W97 那条断言补更多具体串**：那是把假阳性源扩大，且永远补不全。
4. **不拆 shell 段**：实现更简单，但 `&&` 串起来的第二条判定会**静默逃出**（假阴性，危险方向）。
5. **把新站点塞进 `LOCALLY_UNREACHABLE` 台账**：台账是「在册却不被覆盖」的登记，本测试文件**确实**
   被 `engineering_checks` 跑到 ⇒ 那样记是**不实声明**。

## 关联

- 规格：`openspec/specs/engineering-foundation/spec.md` 的
  「Every registered judging step's target is paid for by parsing its command」
- 报告：[`docs/reports/2026-10-14-w101-step-target-binding.md`](../reports/2026-10-14-w101-step-target-binding.md)
- 变更：`openspec/changes/archive/2026-10-14-w101-step-target-binding/`
- 记忆：`.workbuddy/memory/KNOWLEDGE.md` §6.24；`AGENTS.md` 的 W101 段
