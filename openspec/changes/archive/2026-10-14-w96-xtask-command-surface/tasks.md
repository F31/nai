# w96 任务清单

- [x] 核实并裁定：`execute()` 的命令面是否被判据守住。三个探针：①加一条臂 ⇒ 本机 4 套件全绿零信号；②删掉 `check --secrets` 臂 ⇒ 同样全绿；③usage 字符串加一条不存在的命令 ⇒ 同样全绿
- [x] 核实无替代守卫：`grep -rn "Usage:" tests/` 为空；`grep` 证实没有任何测试读 match 臂
- [x] 派生实测：纯 JS 探针解析 `execute()` 的臂与 usage 字符串，确认 `dispatch (24)` / `usage (21)` / `usage \ dispatch = []` / `dispatch \ usage = ['help','--help','']`
- [x] 派生实测：用括号计数法确定**门控区域**（`engineering_checks` 体 + 两条 `check --profile` arm 体），逐条验证 5 个 `gate` 候选落点均命中、4 个 `standalone` 执行体均 miss
- [x] 建立 OpenSpec 变更 `2026-10-14-w96-xtask-command-surface`（proposal / design / spec 差量 / tasks），`validate --strict` **valid**
- [x] 实现 `scripts/contracts/xtask-commands.mjs`：`XTASK_HOST` / `COMMAND_KINDS` / `STANDALONE_REASONS` / `HELP_SIGNATURES` / `XTASK_COMMANDS`（24 条），以及 `dispatchSignatures` / `usageSignatures` / `gateRegions` / `registryShapeErrors` / `commandErrors`
- [x] 新建 `tests/contracts/xtask-commands.test.mjs`（17 个用例），并在 `xtask/src/lib.rs` 的 `engineering_checks` 登记该套件
- [x] 在 `test-reachability.mjs` 的 `TEST_REGISTRATION_SITES` 登记新套件的站点（8 → 9）
- [x] 在 `tests/registry.test.mjs` 的逐字站点清单补上新站点的 id（同一行为：清单与闭世界同时红）
- [x] 非空洞注入 P1–P6（真实文件 + `sha256` 逐字还原 ALL-OK，每个注入都产生具名错误）＋ 17 个用例内的 F 级注入
- [x] 逐条核对与 W95 / `test-reachability` 的边界：三者互不顶替（实测三组注入的红/绿分布）
- [x] 写 ADR 0074 与活文档同步（`AGENTS.md` / `docs/development-plan.md` / `KNOWLEDGE.md` §6.19）
- [x] 写轮次报告并跑全量门控（`check --profile full`）与两条 `openspec validate`
- [x] 归档、一次提交推送、单次 CI 观测并回填报告
