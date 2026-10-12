# 设计（w75）

## 0. 核实结论先行

W74 的守卫把「扫描工作区」当成「扫描仓库」，靠**结构性收敛**（跳 dot 段 + 具名排除）缩小差距，
并把剩下的缺口写成「CI 上不存在」的残余风险。**这个差距可以关掉**：`.git/index` 是纯 JS 可读的
DIRC 文件，它给出的**路径集**就是「已跟踪文件集」，而「未跟踪」正是 F2 的入口。

⇒ 本轮把判据的输入从「工作区里匹配扩展名的文件」改成「**工作区里匹配扩展名、且已跟踪的文件**」，
F2 的入口在结构上消失。

## 1. 决策：仓库文件集由 `.git/index` 纯 JS 取得

**取**：新增 `scripts/contracts/git-index.mjs`，导出 `readTrackedPaths(root)`：

- 读 `<root>/.git/index`；若 `.git` 是**文件**（worktree/submodule），解析其 `gitdir: <path>` 指针
  再读 `<path>/index`；
- 解析 DIRC 头（magic `DIRC` + 版本 + 条目数）与条目（v2/v3 固定 62 字节头 + NUL 结尾名 + 8 字节对齐；
  v4 的条目名是「与前一条的公共前缀长度 varint + 后缀」）；
- 返回 `{ok: true, paths: Set<string>, version}` 或 `{ok: false, reason}`，
  `reason` **区分四类**：`missing`（`.git`/`index` 不在）、`bad-magic`、`unsupported-version`、`truncated`。

**理由**：

- **零子进程**——宿主禁止 node 派生任何子进程（W74 实测：`git`/`process.execPath`/`cmd.exe`/`sh`
  一律 `EBUSY`），而读一个文件不受此约束；
- **自包含**——`node --test tests/contracts/schema.test.mjs` 单跑也能行使判据，不依赖别的步骤先跑；
- **格式稳定**——DIRC v2 是 Git 二十年默认（`.git/index` 实测 `version=2`），且解析器自带字节流
  单测覆盖 v2/v3/v4 与四类失败，脆弱性被测试约束在明处。

**否证过的备选**：

- *`git ls-files --cached --others --exclude-standard`*（xtask 侧已在用的写法）：**修不了 F2**——
  `--others` 恰恰把**未跟踪**文件收进来，那个伪造消费者也在集合里。它回答的是「git 会发布什么」，
  不是「仓库已承诺什么」。
- *由 xtask（Rust）导出清单文件供 node 读*：Rust 可以派生子进程，所以技术上可行，但引入
  **顺序耦合**（node 测试必须在 xtask 之后）与**陈旧产物**（单独跑 node 时读到的可能是上一轮的清单，
  而「守卫读到陈旧输入」正是本轨道反复处理的失效形态）。**输入的自包含性比复用一次 `git` 调用值钱。**
- *不做，保留 W74 的结构性收敛*：判据的输入**不等于**被断言的对象，是这条守卫的根本前提；
  W74 §八遗留 1 自己也把它记为最严重缺陷的残余。既然有了关掉它的纯 JS 路径，「继续登记」就不是
  唯一的负责任选项。
- *解析失败时降级为「通过并打印一行」*：会让保护在不知情时消失（W73 的「假契约」同族）。
  前提缺失必须可见。

## 2. 决策：判据是「消费证据的来源 ⊆ 已跟踪路径集」

**取**：`consumptionOf(files)` 除返回 `consumed`（schema → 来源标签）外，再返回
`evidenceSources`（**提供了消费证据的文件**集合）。新断言：

```
repositoryErrors([...evidenceSources], tracked).length === 0
```

`repositoryErrors` 对每个不在 `tracked` 里的证据来源报错并**指名**它。

**为什么约束「证据来源」而不是「扫描集」**（这一条决定了闭环是否被打断）：

- 若断言「扫描集 ⊆ index」，那么**任何**未 `git add` 的代码文件都会让门控红——而本程序的闭环
  顺序是「门控 → 勾 tasks → 归档 → 校验 → 提交」，**提交时才 `git add`** ⇒ 新变更目录里的
  `.yaml`（如 `.openspec.yaml`）会让门控假红。那会**改变工作流**。
- 约束「证据来源」只命中**真正读取 schema 的文件**（`scripts/contracts/*.mjs`、`xtask/src/*.rs`、
  `apps/**/*.rs` 等），它们**都是已跟踪的老文件** ⇒ 零假阳性。
- 而 F2 的攻击面**恰好就是证据来源**：伪造消费者之所以能洗绿，靠的是它**提供了证据**。
  约束证据来源 = 精确关掉那个入口。

**非空洞**：`evidenceSources` MUST 非空（否则「来源 ⊆ index」对空集恒真）。实测它含
`packages/nai-contracts/src/validation.rs` 等 Rust 文件（`include_str!` 的落点）。

## 3. 决策：扫描集路径是 POSIX 相对路径（修 F6）

**取**：`codeFiles()` 的路径构造改为不产生 `./` 前缀（初始目录 `'.'` 时直接拼 `entry.name`）。
断言：扫描集里**没有**任何以 `./` 开头的路径。

**理由**：F6 让「扫描集」与「index 路径集」（`apps/engine/src/lib.rs` 这种，无前缀）**不可比**。
不修它，决策 2 的断言会对根目录文件（如 `pnpm-workspace.yaml`）**静默假阴性**。
F6 当前无可观测影响，正因为**还没有任何判据做过这个比较**——这不能当作「不是缺陷」的证据
（W37 的教训：实测钉住的差异也要问「该不该存在」）。

## 4. 残余风险（如实登记）

1. **依赖 `.git` 内部格式**。缓解：DIRC v2 是二十年默认；解析器自带 v2/v3/v4 与四类失败的字节流单测；
   版本不支持时**明确失败**（不猜测）。
2. **CI 上 `.git/index` 的可读性未在本机验证**。结构性理由：`.github/workflows/foundation.yml` 用
   `actions/checkout@v4`，无 `fetch-depth`/`index.version` 配置 ⇒ Git 默认 v2；**CI 观测时核对**
   （本轮 push 后单次查询）。若 CI 实测失败，失败消息会直接点名四类原因之一。
3. **「已跟踪」是 index 不是 HEAD** ⇒ 已 `git add` 但未提交的文件算「仓库内容」。可接受：
   `git add` 是「意图提交」的显式声明。反之，**已提交但工作区被删**的文件不可能成为证据来源
   （它不在扫描集里，因为磁盘上没有）。
4. **`.github` 仍被结构性排除**（W74 的取舍）：它里面的工作流若成为某 schema 的唯一消费者，
   会被报成孤儿（假阳性，安全方向），处置是加一条具名例外，而不是恢复枚举式扫描。
