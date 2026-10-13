# w95 设计

## 一、问题形状

CI 工作流是**判定面**的第二个执行入口（第一个是本机 `cargo xtask check`）。它比本机门控
**多跑** 9 个判定步，而这些步**没有任何在册台账**：

```
[07] cargo xtask schema --check                                   ← 本机 EBUSY
[10] cargo xtask test --suite storage                             ← 含 Windows-only 的 native vault
[11] cargo xtask test --suite kernel
[12] cargo xtask test --suite hosts
[13] cargo xtask test --suite session
[14] cargo xtask test --suite scheduler
[15] cargo xtask test --suite provider
[16] cargo xtask test --suite knowledge
[17] node --test tests/contracts/schema.test.mjs tests/contracts/descriptor.test.mjs
                                                                  ← descriptor 只有 CI 跑
[18] cargo xtask test --suite contracts (if: runner.os == 'Linux')  ← 需要 Linux C++ 工具链
[19] pnpm spec:validate
[20] pnpm exec openspec validate --archived --no-interactive
```

（上表标了「本机为什么跑不了」，标注本身**不在任何判据里**——它就是要补的东西。）

同族的既有先例：W84 给「**测试文件**有没有被接上电」补了闭世界（`test-reachability.mjs`），
W86 给「**OpenAPI 文档集**」补了闭世界。本轮给「**workflow 的 step**」补——同一手法，
**新的一层对象**。

## 二、证据（两个探针，都真跑）

`sha256` 改前记录、`git checkout --` 还原、逐字校验。

| 探针 | 改动 | 结果 |
| --- | --- | --- |
| P-加法 | 在 `pnpm spec:validate` 之前插入 `- name: W95 probe` / `run: node -e "process.stdout.write('probe\n')"` | schema **30/30**、platform **22/22**、registry **52/52** —— **全绿、零信号** |
| P-减法 | 删掉 `- name: Check generated contracts without modifying consumers` + `run: cargo xtask schema --check` | 同样 **30/30、22/22、52/52** —— **一个判定静默消失** |

增补核实：`grep -rn "foundation.yml" crates/ xtask/` **为空** ⇒ 编译侧不读 workflow，
不存在「Rust 测试会碰到它」的替代守卫。`test-reachability.mjs` 只从 workflow 里读**一个 marker**
（`tests/contracts/descriptor.test.mjs`），workflow 其余 20 个 step 完全不在受判范围。

## 三、设计决定

### 决定 1：静态声明，**不**做运行时探测

「这一步本机能不能跑」**不能**由判据在运行时探测（试跑一下、看有没有 `EBUSY`）。理由：
- 宿主瞬态 MUST NOT 改变被测系统的结论（**铁律二**）。`EBUSY` 是宿主事实，换一台机器
  同一个仓库会得出不同分类 ⇒ 判据不再可复现。
- 「能跑但没接进门控」（`pnpm spec:validate` 本机手动可跑）与「根本跑不了」（`schema --check`）
  **风险不同、成因不同**，而运行时探测区分不出这两者。

⇒ 分类是**人的主张，逐条在册**；判据负责验「主张有没有付代价」。

### 决定 2：`local` 必须引出一处**真的存在**的门控落点

`kind: 'local'` 的每条带 `{file, marker}`，marker MUST 在该文件里**逐字存在**：

- step 08（`check --profile full`）⇒ `{file: 'xtask/src/lib.rs', marker: '["check", "--profile", "full"]'}`
- step 10–16（7 个 suite）⇒ `{file: 'xtask/src/lib.rs', marker: '&["test", "--workspace", "--locked", "--exclude", HARNESS]'}`
  （本机 `engineering_checks` 跑的是**全 workspace** 测试 ⇒ 这 7 个 suite 是它的**子集**）

⇒ 「本机也跑」从一句话变成一个**可判定**的引用。**如实披露**：它验的是「那处落点存在」，
**不**验「那处落点真的覆盖了这个 step」（那是语义，仍由人读）。假阳性方向（把已覆盖的报成
未覆盖）安全。

### 决定 3：`ci-only` 的成因取自**闭集**

未知成因 ⇒ 具名失败。四个成因按**实测**区分：

| 成因 | 含义 | 实例 |
| --- | --- | --- |
| `host-subprocess` | 需要派生子进程，宿主对该步 `EBUSY` | step 07 `schema --check` |
| `compiled-artifact` | 需要编译产物（descriptor／native） | step 17 的 `descriptor.test.mjs` |
| `platform-toolchain` | 需要特定平台工具链 | step 18 `--suite contracts`（Linux C++） |
| `not-in-profile` | 本机**能**跑，但没接进 `check` profile | step 19／20（`spec:validate`／`openspec --archived`） |

成因闭集而非自由文本，理由与 `UNWIRED_CAUSES` 同形：**一个未知的成因是一个标签，不是一个理由**。
新增一类成因 ⇒ 必须显式扩这张表。

### 决定 4：双向字段，触发条件只由「字段存在」决定

`local` MUST 有 `localEvidence`、`ci-only` MUST NOT 有（W83）。若允许 `ci-only` 也带落点，
「它是不是本机也跑」就没有单一回答（W88 的「谁在管它」）。
`diagnostic` MUST 对应一个带 `if` 的 step——诊断通道**不产生独立判定**，这一点由 workflow
的**结构**（解析出来的 `if` 字段）验证，不由名字验证。

### 决定 5：步的**身份**由解析得出，MUST 唯一

判定单元是**步的身份**，不是它的名字（W91 教训：「同名」≠「同一个」）。身份规则：
有 `name` 用 `name`；无 `name` 用 `uses` 值或 `run` 首行。三条步无 `name`
（`actions/checkout@v4`／`actions/setup-node@v4`／`pnpm/action-setup@v4`），两条无 `name` 的 run 步
（`pnpm spec:validate`／`pnpm exec openspec validate …`）。身份 MUST 唯一，否则具名失败。

## 四、与 `test-reachability` 的边界（同一件事 MUST NOT 两处定义）

两张表都以 `.github/workflows/foundation.yml` 为 host，但**断言的单元不同**：

| 模块 | 断言的对象 | 独有覆盖 |
| --- | --- | --- |
| `test-reachability.mjs` | **测试文件**（`.test.mjs`）有没有被某个站点接上电 | 「仓库里的测试文件集 == 覆盖 ∪ 未覆盖台账」；**不管** workflow 的 step |
| `workflow-steps.mjs`（本轮） | **workflow 的 step** 有没有被分类在册 | 「workflow 的 step 集 == 在册集」；**不管**测试文件的可达性 |

⇒ 二者**不互相顶替**：删掉 `test-reachability` 的站点，本轮的判据不会红；反之亦然。
（本轮报告的 §五 逐条核对这条。）

## 五、注入清单

| 编号 | 注入 | 期望 |
| --- | --- | --- |
| F1 | 往 workflow 插入一个未登记的 step | 红（具名该 step） |
| F2 | 从 workflow 删掉一个已登记的判定 step | 红（具名该 step 消失） |
| F3 | 把某 step 改名 | 红（旧名不在、新名不在册） |
| F4 | `kind: 'local'` 的 `localEvidence.marker` 改成不存在的串 | 红（假契约） |
| F5 | `kind: 'ci-only'` 的 `reason` 改成域外值 | 红（域外成因） |
| F6 | `kind: 'ci-only'` 挂上 `localEvidence` | 红（双向） |
| F7 | 反向：合法登记原样 | **不**红 |
| F8 | 文件层：新增一份未登记的 workflow 文件 | 红（具名该文件） |
| P1 | 复现 P-加法探针 | 红（改前绿） |
| P2 | 复现 P-减法探针 | 红（改前绿） |

每次注入后 `sha256` 逐字还原（`python` 写文件用 `newline=''`，W92 的 CRLF 假差异教训）。

## 六、明确不做

- **不验「CI 真的跑了某步」**：那要读运行结果，不是静态判据的事。
- **不验 `ci-only` 的成因是否成立**：语义，仍由人读（与 ADR 0054 决策 2 同族的边界）。
- **不改 `foundation.yml` 一个字符**：本轮只补判据与登记，不动被审对象。
- **不把「本机能否跑」写进判据的输入**：铁律二。
