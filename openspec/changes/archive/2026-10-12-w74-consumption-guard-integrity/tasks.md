# 任务（w74）

## 1. 立项与核实

- [x] 1.1 复核 W73 消费守卫的三条声称（F1 注入 V1、F2 注入 V2b、F3 扩展名与注释语法普查）
- [x] 1.2 实测宿主子进程约束（`git` / `process.execPath` / `cmd.exe` / `sh` 全 EBUSY）并订正既有归因
- [x] 1.3 实测扫描集事实（跳 dot 与不跳 dot 的差集 281/282、唯一差异 `foundation.yml`；两条来源各自的独有覆盖 Ajv 5 / Rust 1；生成物 basename 集合恰 10）
- [x] 1.4 建立本变更（`.openspec.yaml` / proposal / design / spec 差量 / tasks）

## 2. 实现

- [x] 2.1 `scripts/contracts/validation.mjs` 导出 `TS_GENERATED_SOURCES`（生成面，MUST ⊆ 校验面）
- [x] 2.2 `scripts/contracts/generate.mjs` 删除硬编码 `jsonSources`，改为导入 `TS_GENERATED_SOURCES`
- [x] 2.3 扫描集结构化为「跳过所有以 `.` 开头的目录 + 4 条具名排除」，每条排除 MUST 被 `.gitignore` 忽略
- [x] 2.4 语言表 `CODE_LANGUAGES` 派生 `CODE_EXTENSIONS`；`stripCommentLines` 按语言取注释标记
- [x] 2.5 移除路径字面量来源；来源收为两条，判据从「各自都被行使」换成「各有独有覆盖」
- [x] 2.6 新增「排除无害」（跳 dot vs 不跳 dot 消费集合相同）与「生成面/校验面同清单」断言

## 3. 非空洞注入验证（每组守卫分别开火）

- [x] 3.1 独有覆盖：清空 `AJV_SOURCES` ⇒ 恰 5 份（`catalog`/`binding-plan`/`platforms`/`plugin/ui`/`tests/registry`）变孤儿 + 该来源被指名为零独有；让 `RUST_INCLUDE` 永不匹配 ⇒ `upgrade-package` 恰成孤儿
- [x] 3.2 排除无害：在 dot 目录放含 `include_str!` 的 `.rs` ⇒ 集合相等断言变红（`deepStrictEqual` 差异直指该 schema）
- [x] 3.3 排除付理由：加一条不在 `.gitignore` 里的具名排除 `vendor` ⇒ 红
- [x] 3.4 语言闭包：把 `.yaml` 换成不存在的扩展名 ⇒ `not ok 13` 与 `not ok 14` 两处同时红
- [x] 3.5 生成面：TS 清单多一条 ⇒「生成但未校验」+「清单有而目录无」两条同时开火
- [x] 3.6 记录「注入 → 被哪条断言抓住」的对照表（报告 §五）；**外加**重现 W73 洗绿攻击：真孤儿 + 未跟踪伪造消费者 ⇒ **现在仍被抓**（W73 时 13/13 全绿）

## 4. 文档与规格

- [x] 4.1 写 ADR 0052（扫描集形状、语言闭包、来源自负其责、生成清单单一来源）
- [x] 4.2 订正既有文档里「`schema --check` 的 EBUSY 归因于 node 自旋 node」的说法（活文档 `AGENTS.md` 就地订正；五份历史报告 `w65`/`w67`/`w68`/`w69`/`w71` 保留原文 + 追加勘误段）
- [x] 4.3 同步活文档（`AGENTS.md` 新增 W74 段、`docs/development-plan.md` 新增 W74 段与轮次摘要条目）
- [x] 4.4 写报告 `docs/reports/2026-10-12-w74-consumption-guard-integrity.md`

## 5. 闭环（本轮执行记录；每轮固定流程，不构成交付任务 ⇒ 不设勾选项）

- 5.1 `cargo xtask check --profile full` **EXIT=0**（目标 **91 = 73 `Running` + 18 `Doc-tests`**、**858 passed / 0 failed**，与 W73 逐项相同）
- 5.2 `tasks.md` 的交付项（第 1–4 段）全部勾选
- 5.3 `openspec archive --yes` ⇒ 归档为 `2026-10-12-w74-consumption-guard-integrity`（`+2 added / ~1 modified`）
- 5.4 两条 validate：`--all --strict` = **43**；`--all --strict --archived` = **72**
- 5.5 清理 `dist/` 下的临时探针与备份
- 5.6 一次 commit + push（提交信息经 UTF-8 文件传入）
- 5.7 CI 单次低频观测并回填报告 §七（该回填提交自身的 run 不回填）
- 5.8 记忆收尾（当日 log、`KNOWLEDGE.md`、`MEMORY.md`）

> 这一段在 W73 的 `tasks.md` 里不存在（它只有交付段）。W74 首次把它写成勾选项，归档门控随即以
> 「6 incomplete tasks」拒绝——**流程项无法在提交前全部真实完成**（提交、CI 观测、记忆收尾都在提交之后），
> 把它们做成勾选项只会逼出**不实勾选**。改为无勾选项的**执行记录**。

