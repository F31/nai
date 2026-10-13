# W98：npm 脚本的执行面必须被守卫

- **日期**：2026-10-14
- **轮次**：W98
- **变更**：`openspec/changes/2026-10-14-w98-npm-script-surface/`
- **ADR**：[0076](../adr/0076-npm-script-surface.md)
- **母题**：执行面的集合守卫（第七次换对象）

## 一、目标

前四轮各管一层执行面：W95 管 CI 的 **step 集合**、W96 管 `cargo xtask` 的**命令面**、W97 管
Rust 侧的**包集合**。本轮换到**第四层对象**——**npm 脚本**：「每个脚本**谁调用它**？」

## 二、缝在哪

仓库有 **3 份 `package.json`、13 个脚本**（根 7、`packages/contracts-ts` 2、`apps/desktop` 4）。
实测出的现状：

| `(包目录, 脚本名)` | 处置 | 消费者／成因 |
| --- | --- | --- |
| `packages/contracts-ts:typecheck` / `:build` | invoked | 驱动表 STEPS |
| `apps/desktop:typecheck` / `:build` / `:test` | invoked | 驱动表 STEPS |
| `. :test:ui` | invoked | 驱动表 STEPS（`.` → `test:ui`） |
| `. :spec:validate` | invoked | `.github/workflows/foundation.yml:83` |
| `. :typecheck` / `:build` | orphan | `covered-by-driver`（体是 `pnpm --recursive …`） |
| `. :check:frontend` | orphan | `manual-alias`（`frontend.rs` 的 `SCRIPT` 常量） |
| `. :test:control-smoke` | orphan | `manual`（`docs/control.md`） |
| `. :test:engine-smoke` | orphan | `manual`（`docs/engine.md`） |
| `apps/desktop:dev` | orphan | `manual`（`docs/engineering.md`） |

**7 invoked / 6 orphan**。引用解析出 3 条真引用（CI 1 条 + 根包两条递归委托，各自指向两个子包）。

### 2.1 三个 Mutation（改真文件 → 真跑现有 6 个 portable 套件 → 备份拷回 → `sha256` 逐字还原）

| 编号 | 改动 | 结果 |
| --- | --- | --- |
| M2 | 根 `package.json` 加 `"typecheck:fast": "node scripts/nowhere.mjs"` | **158 passed / 0 failed——零信号** |
| M3 | `frontend-gate.mjs` 的 `STEPS` 里 `['apps/desktop','test']` → `'tests'` | **158 passed / 0 failed——零信号** |
| M5 | 根 `package.json` 加 `"test:loop": "pnpm test:loop"`（自引用） | **158 passed / 0 failed——零信号** |

### 2.2 如实记录的反例（M1）

把 CI 的 `pnpm spec:validate` 拼错成 `spec:validat` ⇒ **2 failed**。抓到它的是 ADR 0073 的
`localEvidence: {file: '.github/workflows/foundation.yml', marker: 'pnpm spec:validate'}` 逐字比对。
**但它抓的是「那一行文本变了」，不是「那个脚本没有消费者了」**——同一处文本、两种语义。
⇒ 它与本轮的判据**不构成替代关系**（边界实测见 §五）。

### 2.3 「注释不是引用」的实测

以 `pnpm` 为模式扫全仓，原始正则命中 **4 行**；剥注释后**只有 1 行**是真引用：

| 命中处 | 真伪 |
| --- | --- |
| `.github/workflows/foundation.yml:83` | **真引用** |
| `scripts/contracts/workflow-steps.mjs`（文档注释里的举例） | 注释 |
| `tests/contracts/workflow.test.mjs`（登记表里的字符串数据） | 数据，非调用 |
| `tests/registry.yaml:115`（`notes` 里的散文） | 散文 |

## 三、判据设计

`scripts/contracts/npm-scripts.mjs`，六条判据：

- **V1 双向闭世界**：`(包目录, 脚本名)` ↔ `SCRIPT_DISPOSITIONS` 两向一致（新增脚本没有处置／
  在册脚本已消失，两侧都点名）。
- **V2 引用可解析**：`pnpm X` / `pnpm run X` / `pnpm --filter P X` / `pnpm --recursive X` MUST 落到
  **存在**的脚本上，`P` MUST 是真包名；**自引用不算**。
- **V3 `invoked` 付代价**：`{file, marker}` 在**剥注释后**逐字存在；`driver` 形态则要求
  `{dir, script}` **真的是驱动表里的一个 STEPS 项**（判定域 = 解析结果，不是全文 `.includes()`）。
- **V4 `orphan` 付代价**：成因落闭集，且三格各自付自己的代价（判定域两两不同）。
- **V5 驱动表接线**：STEPS 每项落在该包 `scripts` 里，首 token 在 `KNOWN_EXECUTABLES` 内，
  体不含 `SHELL_SYNTAX` —— **把 `frontend-gate.mjs` 的运行时拒绝静态化**。
- **V6 成因闭集非空**。

### 3.1 三个设计要点

1. **判定单元是 `(包目录, 脚本名)` 配对**（W91）：`typecheck` 在三个包里同名、`build` 在两个包里同名。
   实测：删掉 `apps/desktop:typecheck` 一条 ⇒ 恰好 **1** 条错误，且**不含** `contracts-ts`。
2. **文档不是调用**：`.md` MUST NOT 进引用扫描面。`manual` 成因的证据 MUST 是 `.md` 且逐字存在
   ——**门控与判据都执行不了的东西，其可用性只能由文档背书**。
3. **自引用 MUST NOT 充当消费者**：全 `package.json` 自动并入扫描面（脚本互相调用是真实的），
   而根包的 `"typecheck": "pnpm --recursive typecheck"` 正是一条**跨包**引用 ⇒ **自引用排除与
   这个并入是一件事的两面**。实测：`"loop": "pnpm loop"` 不再能给自己签名。

## 四、实现

- `scripts/contracts/npm-scripts.mjs`：`packageScripts`／`driverSteps`／`driverExecutables`／
  `driverShellSyntax`／`pnpmReferences`／`workspaceGlobs`／`stripSlashComments`／`stripYamlComments`／
  `scriptErrors`。
- `tests/contracts/npm-scripts.test.mjs`：**34 个用例**。
- 接线：`xtask/src/lib.rs` 的 `engineering_checks` **+1 行登记**（含注释 3 行）；
  `test-reachability.mjs` 站点 **10 → 11**；`registry.test.mjs` 逐字清单同步（**如期变红**）。
- **被审对象一个字符未动**：3 份 `package.json`、`scripts/frontend-gate.mjs`、workflow。

### 4.1 两条判据缺陷是测试抓出来的

| 缺陷 | 后果 | 修法 |
| --- | --- | --- |
| 引用正则贪婪 `[^\n]*`：一行里 `pnpm a && pnpm b` 时第二条被吞进第一条 token | 第二条引用**静默消失**（**假阴性，危险方向**） | 改 `[^\n&|;]*`（在 `&&`／`\|\|`／`\|`／`;` 处停） |
| `packages` 解析失败后仍被 `packages.keys()` 访问 | 崩（`TypeError`）而非具名失败 | 补 `packages !== null` 守卫 + `scriptBody` 的类型守卫 |

另有一处**方向性修正**：`pnpm` 子命令黑名单曾把 `test`／`start`／`build` 一起收进去，而它们是 npm 对
`scripts.<name>` 的**别名** ⇒ 会**漏引用**（同样是假阴性方向）⇒ 只留真子命令
（`install`／`exec`／`dlx`／…）。

## 五、非空洞验证

### 5.1 注入（真改文件 → 真跑 → 备份拷回 → `sha256` 逐字还原）

**P1–P15 全部红且逐字还原**（`restore OK` ×15）。代表性具名错误：

| 编号 | 注入 | 具名错误（节选） |
| --- | --- | --- |
| P1 | 加未在册脚本 | `the root manifest declares the script "typecheck:fast", which nobody dispositions` |
| P2 | 删 CI 的 `- run: pnpm spec:validate` | `cites .github/workflows/foundation.yml for "pnpm spec:validate", which is not there once comments are stripped` |
| P3 | 驱动表 script 名改错 | `runs apps/desktop → "tests", which that package does not declare — the gate would fail at run time; it must fail here` |
| P5 | 覆盖项改成 `build` | `delegates to \`pnpm --recursive typecheck\` yet claims coverage by a "build" step — a recursive delegation only covers the script of the same name` |
| P6 | `manual` 的证据 marker 不存在 | `a manual script with no document is a script nobody knows how to run` |
| P8 | 驱动表项体加 `\|` | `which uses the shell syntax \| that the gate does not implement — the gate refuses it at run time; it must be refused here` |
| P9 | CI 里拼错脚本名 | `runs \`pnpm spec:validat\`, but . declares no script "spec:validat" — a reference to a script that does not exist is a check that never runs` |
| P10 | `--filter` 指向幽灵包 | `filters on the package @nai/ghost, which no manifest declares — a filter that resolves to nothing must not be read as "no consumer"` |
| P13 | `invoked` 的 marker 加尾巴 | `cites … for "pnpm spec:validate --all", which is not there once comments are stripped` |
| P15 | 别名的等价落点改错 | `cites xtask/src/frontend.rs for "…scripts/ghost.mjs…", which is not there — the alias has no equivalent` |

### 5.2 边界（互不顶替）

| 注入 | 本模块 | W95 workflow | W96 xcmd | W97 gcov | W84 registry |
| --- | --- | --- | --- | --- | --- |
| b1 删 `SCRIPT_DISPOSITIONS` 一条 | **4** | 0 | 0 | 0 | 0 |
| b2 改 `WORKFLOW_STEPS` 的 identity | 0 | **2** | 0 | 0 | 0 |
| b3 删 `XTASK_COMMANDS` 一条 | 0 | 0 | **4** | 0 | 0 |
| b4 改 `TEST_SUITES` 一条 | 0 | 0 | 0 | **3** | 0 |
| b5 删新站点 | 0 | 0 | 0 | 0 | **2** |

⇒ **五者完全互不顶替**。M1（§2.2）揭示的唯一交叠点是 workflow 里那行**文本**，语义与判定域都不同。

### 5.3 基线

七个 portable 套件合跑 **192 passed / 0 failed**（W97 为 158，本轮 +34）。
工作区与 index 无差异；9 个被注入文件与备份**逐字节一致**。

## 六、门控与 validate

### 6.1 七套件分布

registry **52** / schema **30** / platform **22** / workflow **13** / xtask-commands **17** /
gate-coverage **24** / **npm-scripts 34（本轮新增）** = **192**。

### 6.2 全量门控

`cargo xtask check --profile full` —— **EXIT=0**（5m24s）：

| 项 | 结果 |
| --- | --- |
| Rust Running 目标 | **74** |
| Rust Doc-tests | **18** |
| `test result:` 行 | **91 / 全部 0 failed** |
| passed 合计 / failed | **862 / 0** |
| `LNK1104` 出现次数 | **0** |
| Node 套件 | 52 / 30 / 22 / 13 / 17 / 24 / **34** / 5 / 52（**9 套，全 0 fail**） |
| 前端门控 | `[frontend] 全部步骤通过（工程基础，非渲染/人工 UI 验收）` |

### 6.3 两条 validate

| 项 | 结果 |
| --- | --- |
| `openspec validate --all --strict` | **43 passed / 0 failed** |
| `openspec validate --all --strict --archived` | **96 passed / 0 failed** |

归档目录数 95 → **96**（`2026-10-14-w98-npm-script-surface`，与源同名，无跨零点改名）。
`archive --yes` 报 **`Task status: ✓ Complete`**，并给出**本轮确存在**的非阻塞警告
`CHANGE_WHY_TOO_LONG`（Why 段超 1000 字符）——**现测所得，不照抄上一轮**。

### 6.4 门控 / 归档 / validate 三件齐

- 门控 `EXIT=0`（§6.2）；归档 `EXIT=0`；两条 validate `EXIT_A=0 / EXIT_B=0`。
- 规格 `engineering-foundation` 变更量：**+1 added / ~0 / -0 / →0**（一条 ADDED Requirement）。

## 七、CI

（推送后的单次低频查询结果回填于此：唯一查询参数破坏缓存 + `Cache-Control: no-cache` +
回读 `head_sha` == 推送 SHA。）

## 八、遗留

1. **不验脚本能跑通**：那要执行（`pnpm` 在宿主上不可用），且**铁律二**说宿主瞬态 MUST NOT 改变结论。
2. **不验 `pnpm` 引用的执行可达性**：CI 的 `if:` 条件、`continue-on-error` 一概不判。
3. **成因闭集不要求每格都有实例**（只要求非空）：那会让一次合法的成因收敛（某个冒烟脚本被接进
   门控后 `manual` 归零）反而变红（W92 的教训）。当前 3 格都有实例。
4. **`--filter P` 分支当前没有活样本**（见 §九 3）。
5. **`test --suite upgrade` 在 CI 没有具名 step**（W97 §八 1 的发现）——本轮**仍未动**，成因仍无记载。
6. **W90 §八遗留 1／3／4／5、W91 跨语言键级消费、W92 基线收录、W93 锚点唯一性、
   W94 `needs_external` 审计、W95 `ci-only` 成因、W96 五项、W97 六项**均**未动**。

## 九、如实披露

1. **不断言 CI 的执行结果**——那要读运行结果，不是静态判据的事（除非 §七 有运行级证据）。
2. **不验「某个脚本真的按门控的形态被跑过」**：判据验的是「声明与代价」，不是「那次运行的输出」。
3. **`--filter P` 形态在引用扫描面里实例数为 0**：`pnpm --filter @nai/desktop test` 与
   `pnpm --filter @nai/desktop dev` 都只出现在 `docs/**`（文档不在扫描面）⇒ V2 的这条分支当前
   **没有活样本**，证据只有测试文件里的注入用例（P10）。**不写成「已覆盖」**。
4. **`SYNTAX`／`KNOWN_EXECUTABLES` 是从 `frontend-gate.mjs` 解析的**：若该文件改用别的写法
   （例如把常量内联进函数），解析会抛 ⇒ 具名失败（而不是静默失去这半条判据）。
5. **本轮不改任何被测对象的实参**：补的是判据、登记与接线。
