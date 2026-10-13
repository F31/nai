# ADR 0076：npm 脚本的执行面必须被守卫

- **日期**：2026-10-14
- **状态**：已接受（Corresponding change：`openspec/changes/2026-10-14-w98-npm-script-surface`）
- **关联**：ADR 0073（CI 执行面的闭世界）、ADR 0074（本机命令面的闭世界）、ADR 0075（被测面的包集合）、
  W91 的「判定单元 MUST 是身份而非名字」、W92 的空态纪律、W97 的「注释不是实参」

## 背景

仓库里有 **3 份 `package.json`、13 个脚本**（根 7、`packages/contracts-ts` 2、`apps/desktop` 4）。
每个脚本都声称做了某件事，但**没有任何判据**回答「**谁调用它**」：

```
root:spec:validate      ← .github/workflows/foundation.yml 里那一行
root:test:ui            ← scripts/frontend-gate.mjs 的 STEPS 表（`.` → `test:ui`）
root:typecheck / build  ← 无人调用；它们的体是 `pnpm --recursive …`（递归委托）
root:check:frontend     ← W41 报告里的一句散文：「便于人工单独调用同一入口」
root:test:control-smoke ← 只在 docs/control.md 的代码块里（是「说明」，不是「调用」）
root:test:engine-smoke  ← 只在 docs/engine.md 的代码块里
apps/desktop:dev        ← 只在文档里，且没有任何地方说它为什么孤着
```

W97 已经把「被测面的包集合」装上判据，但它的**判定域**是 Rust 侧的 `cargo test` 实参；npm 脚本这一层
完全在外面。三个 Mutation（改真文件 + 真跑现有 6 个 portable 套件）确认它是真缝：

| 编号 | 改动 | 结果 |
| --- | --- | --- |
| M2 | 往根 `package.json` 加 `"typecheck:fast": "node scripts/nowhere.mjs"` | **158 passed / 0 failed——零信号** |
| M3 | `frontend-gate.mjs` 的 `STEPS` 里 `['apps/desktop', 'test']` → `'tests'` | **158 passed / 0 failed——零信号** |
| M5 | 加 `"test:loop": "pnpm test:loop"`（自引用） | **158 passed / 0 failed——零信号** |
| M1（反例） | CI 里 `pnpm spec:validate` → `spec:validat` | **2 failed**——ADR 0073 的 `localEvidence` marker 逐字比对抓到了 |

M1 是本决策**必须如实记录的反例**：ADR 0073 抓的是「**那一行文本变了**」，不是「**那个脚本没有
消费者了**」——同一处文本、两种语义。它不构成本轮判据的替代品。四个陷阱在本层同时出现：

1. **同名≠同一个**（W91）：`typecheck` 在三个包里同名、`build` 在两个包里同名；
2. **注释不是引用**（W97 的教训换一层）：`workflow-steps.mjs` 的文档注释与 `tests/registry.yaml`
   的 `notes` 里都写着 `pnpm …`，它们是**解释**。实测：原始正则命中 4 行，剥注释后**只有 1 行**是真引用；
3. **文档不是调用**：`.md` 代码块里的命令是给**人**抄的，它不执行任何东西；
4. **自引用是恒真的自我满足**：`"test:loop": "pnpm test:loop"` 是一条真引用，但它指向自己。

## 决策

### 决策 1：判定单元是 `(包目录, 脚本名)` 的配对，MUST NOT 是裸脚本名

台账的键是 `dir\u0000script`。`typecheck` 的三条互相独立：删掉其中一条 MUST **只**点名那一条
（测试 `the disposition table keys pairs, not bare script names` 实测：恰好 1 条错误，且不含
`contracts-ts`）。

### 决策 2：文档里的命令 MUST NOT 充当消费者

`.md` **不在**引用扫描面里。`docs/control.md` 与 `docs/engine.md` 里那两行只作为 `orphan` 的
**证据形态**（`docEvidence`）出现。理由：文档只能被「写错」证伪，没有任何机制能证明它被执行
——把文档当消费者，两个从不运行的脚本会永久绿灯。

`manual` 成因的代价因此是「**门控与判据都执行不了的东西，其可用性只能由文档背书**」，且判据
要求 `docEvidence.file` MUST 以 `.md` 结尾（源码不算文档），marker MUST 逐字存在。

### 决策 3：自引用 MUST NOT 充当消费者

`pnpmReferences` 会跳过「引用落在被消费包自己的 `package.json` 里」的那些。这不是防御性代码：
根包的 `"typecheck": "pnpm --recursive typecheck"` 是一条**跨包**引用（指向两个子包），而同一个机制
若不加排除，`root:typecheck` 就会被自己身体里那行消费 ⇒ 恒真。全 `package.json` 自动并入扫描面，
正因为「脚本互相调用是真实的」；**自引用排除与这个并入是一件事的两面**。

### 决策 4：成因闭集按**可判定性**划分，不按语义划分

闭集 3 格，**判定域两两不同**——这是它们可区分的前提：

| 成因 | 判定域 |
| --- | --- |
| `covered-by-driver` | 驱动表**解析出的** `STEPS` 项集合（`{dir, script}` 必须真是其中一项） |
| `manual-alias` | **源码**里的等价入口落点（逐字） |
| `manual` | **文档**里的指引（逐字，且文件必须是 `.md`） |

曾经考虑把「冒烟」与「开发服务器」拆成两格。**不拆**：两者的证据形态都是「文档里的一行」，
拆开会让一次合法的改签（`manual-smoke` → `interactive`）不被任何判据发现 ⇒ 两格不可区分 ⇒
分类退化成标签。合并是按**可判定性**做的。

`covered-by-driver` 另付两条代价：① 脚本体 MUST 是 `pnpm --recursive <X>` 的形态；
② 每个覆盖项的 `script` MUST 同为 `<X>`——递归委托覆盖的是**同名**脚本，`typecheck` 不覆盖 `build`。
没有 ②，「委托」会变成一张万能通行证。

### 决策 5：每种证据的判定域由**解析**得出，不是文件全文

`invoked` 的 `driver` 形态验「`{dir, script}` 真的是一条 STEPS 项」，而不是验
「某段文本在文件里存在」。这是 ADR 0074 的「门控闭包区域」教训的第三次复用：
**marker 可填自己的文本 ⇒ 自我满足**。

### 决策 6：把 `frontend-gate.mjs` 的**运行时**拒绝静态化

`frontend-gate.mjs::resolveInvocation` 会在运行时拒绝未知可执行名与未实现的 shell 语法。这是**好**
设计，但它是运行时的：M3 实测，把 STEPS 里一个 script 名改错在静态判据下**零信号**。

⇒ 静态判据对 `STEPS` 的每一项复算同一件事：`(dir, script)` 落在该包 `scripts` 里、体的第一个 token
在 `KNOWN_EXECUTABLES`（**解析**得出，不手写）内、体里不含 `SHELL_SYNTAX` 的任一记号。

**判定域**（W91：「每个限定 MUST 用反例验证它在拦什么」）：这条**只**对 STEPS 里出现的 6 项成立。
根包其余脚本的头 token 是 `pnpm`／`openspec`／`cargo`，它们**本来就不经** `frontend-gate.mjs`
⇒ 拿这把尺子量它们是用错了对象。测试 `the driver executable limit applies to the driver table only`
把这条反例钉住。

### 决策 7：包目录的闭世界由 `pnpm-workspace.yaml` 与已跟踪的 `package.json` 双向定

`workspace` 声明的每个 glob MUST 匹配到至少一个已跟踪的 `package.json`（空 glob 让它的规则
恒真）；每个已跟踪的 `package.json` 目录 MUST 被某个 glob 匹配（workspace 之外的包不是任何人
构建的）。包目录清单由 **`.git/index`** 派生，MUST NOT 手写——手写的那一份会漂移，而
「新增一个包却没人量它」正是本轮要挡的东西。

## 后果

- **零代码改动**：3 份 `package.json`、`frontend-gate.mjs`、workflow 与被审脚本一个字符不动；
  `xtask/src/lib.rs` 只加一行登记（Rust 零行为改动）。Node 套件 **8 → 9**，
  `test-reachability` 站点 **10 → 11**。
- 新增 `scripts/contracts/npm-scripts.mjs`（判据）与 `tests/contracts/npm-scripts.test.mjs`（34 用例）。
- 判据 6 条：双向闭世界、引用可解析且反向指向真脚本、`invoked` 逐字付代价、`orphan` 成因闭集
  且逐格付代价、驱动表接线、成因闭集非空。
- **注入验证**：P1–P15 全部真改文件、真跑、备份拷回、`sha256` 逐字还原，每条产生**具名**错误；
  五条边界实测与 ADR 0073／0074／0075 及站点清单**互不顶替**（4/2/4/3/2）。
- **两处判据缺陷是本轮测试抓出来的**（不是事后补的）：① 一行里两条命令（`pnpm a && pnpm b`）时
  贪婪正则把第二条吞进第一条 ⇒ 第二条引用**静默消失**（假阴性方向，已改非贪婪）；
  ② `packages` 解析失败后仍被 `packages.keys()` 访问。
- **如实披露**：`--filter P` 形态在引用扫描面里实例数为 **0**（`pnpm --filter @nai/desktop test`
  与 `pnpm --filter @nai/desktop dev` 都只在 `docs/**`）⇒ 该分支当前只有测试文件里的注入用例作证据。

## 替代方案

- **只做闭世界，不做出成因闭集**：M2 的守卫只需要「每个脚本有处置」，但那样「孤儿」会退化成
  一个可以随便填的格子。成因闭集 + 逐格代价把「为什么孤着」变成可复核的声明。
- **把 `.md` 纳入引用扫描面**：会让 `test:control-smoke`／`test:engine-smoke` 立刻变成「有消费者」，
  于是它们真正的问题（不在任何门控闭包里）被一次假绿灯掩盖。**否决**。
- **用运行时探测代替静态分类**（例如真的跑一遍 `pnpm`）：违反**铁律二**——`pnpm` 在宿主上不可用，
  「能不能跑」是宿主事实，换一台机器会对同一个仓库得出不同分类。

## 关联

- 前置：ADR 0073（CI step 集合）、ADR 0074（xtask 命令面）、ADR 0075（被测面包集合）；
  本 ADR 补的是它们都够不着的**第四层对象**：npm 脚本的执行面。
- 母题第七次：`repositoryFiles` → `readTrackedText` → W94 两张事实表 → W95 `gateSources`
  → W96 门控闭包区域 → W97 包集合 → **W98 脚本执行面**。
