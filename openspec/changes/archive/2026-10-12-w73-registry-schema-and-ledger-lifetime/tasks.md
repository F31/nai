# 任务（w73 注册表形状契约接电与台账存活期）

## 0 立项与核实

- [x] 0.1 建立 `openspec/changes/w73-registry-schema-and-ledger-lifetime/`（`.openspec.yaml` / `proposal.md` / `design.md` / `tasks.md` / `specs/`）
- [x] 0.2 核实 `schemas/tests/registry.schema.json` **不被任何执行体消费**（不在 `validation.mjs` 的 `sources`；无 `include_str!`；无 `*.mjs` 读它）
- [x] 0.3 实测当前 `tests/registry.yaml` 对该 schema 的**唯一**违反：`baseline_deviations` 未被声明（其余 88 条 / 六线 / 映射段全通过）
- [x] 0.4 核实三条既有规格要求以「schema 校验」为前提（`Single machine-readable…`、`The new line is not registered in the schema`、W72 的对账需求）
- [x] 0.5 核实 `schemas/` 全部 `*.schema.json` 的消费点，产出孤儿清单（`tests/registry`、`rag-golden/case-set`）
- [x] 0.6 复核三种历史实例的「出现 → 收录」时点（A12–A14 = W56→W65；爬坡 = W67→W71；B07 = W68→W71），确认**零跳过**，并订正 W72 把 A12–A14 读成「跨轮次收录」的表述
- [x] 0.7 量差量需求正文长度（全部 ≤ 500 字符）

## 1 注册表接电

- [x] 1.1 `schemas/tests/registry.schema.json` 补 `baseline_deviations` 定义（含 `since_baseline` / `carried_at`），字段级 `additionalProperties: false`
- [x] 1.2 `scripts/contracts/validation.mjs` 导出 `AJV_SOURCES` 并把注册表列入（`registry: 'tests/registry'`）
- [x] 1.3 `scripts/contracts/registry.mjs` 导出 `STATUSES`
- [x] 1.4 `tests/registry.test.mjs` 断言注册表满足其 schema（读 YAML → Ajv）
- [x] 1.5 形状层与语义层三处常量的一致性断言（id pattern / 发布线枚举 / 状态枚举），失败时指名是哪一处

## 2 台账存活期

- [x] 2.1 `baseline.mjs` 的 `normalizeDeviations` 扩为 `{id, since_baseline, registered, carried_at?, reason}` 并做形状自检
- [x] 2.2 `reconcile(registry, parsed, version)` 增**必需**版本入参；缺失或形状非法时具名报错（MUST NOT 静默跳过）
- [x] 2.3 跨版不变式：条目的 `since_baseline` MUST 等于当前基线版本；失败时同时给出条目 id 与两侧版本
- [x] 2.4 `carried_at` MUST 大于 `registered`
- [x] 2.5 更新 `tests/registry.test.mjs` 全部 `reconcile` 调用点传入版本
- [x] 2.6 `tests/registry.yaml` 的台账段注释写明形状与「跨版必须显式结转」

## 3 schema 消费守卫

- [x] 3.1 `tests/contracts/schema.test.mjs` 新增守卫：`schemas/**/*.schema.json` 全集 == 被消费的 ∪ 具名例外
- [x] 3.2 三条派生来源（`AJV_SOURCES` / Rust `include_str!` / `*.mjs` 字面量），扫描只认代码文件
- [x] 3.3 具名例外恰一条（`rag-golden/case-set`），每条 MUST 附非空理由
- [x] 3.4 非空洞：扫描结果必须非空，且 MUST 命中一个已知的 Rust `include_str!` 实例

## 4 非空洞注入验证

- [x] 4.1 schema 侧注入 **8 组**（未知顶层键 / 未知字段 / 坏 id / 坏线 / 坏状态 / 空 acceptance / 少一个 D 键 / 缺台账键）
- [x] 4.2 一致性断言注入 **3 组**（改小 `ID_RE` / 删一半发布线枚举 / 改状态枚举）
- [x] 4.3 台账侧注入 **8 组**（含 6 组必须失败 + 2 组必须通过：合法结转、相称条目）
- [x] 4.4 消费守卫注入 **8 组**（删消费 / 已消费的加例外 / 例外缺理由 / 例外指向不存在文件 ⇒ 失败；整行 `//`、块注释续行、workflow `#` 注释不算消费、代码路径仍算 ⇒ 通过）
- [x] 4.5 记录每组**确实**被对应守卫抓住，且失败信息**指名**对象

## 5 ADR 与规格

- [x] 5.1 `docs/adr/0051-registry-schema-wiring-and-ledger-lifetime.md`
- [x] 5.2 `admission-registry`：MODIFIED「Single machine-readable admission registry」（schema 校验真的跑）、MODIFIED 对账需求（台账形状 + 跨版不变式）、ADDED「形状层与语义层只有一份定义」
- [x] 5.3 `versioned-contracts`：ADDED「每份 schema 要么被消费要么被具名声明」
- [x] 5.4 `spec-driven-delivery`：ADDED「已登记的偏差不跨基线版本存活」
- [x] 5.5 **不改**基线正文与 `tests/registry.yaml` 的任何 `acceptance` 条目

## 6 活文档与报告

- [x] 6.1 `AGENTS.md` 增 W73 段；`docs/development-plan.md` 增 W73 条目
- [x] 6.2 `docs/reports/2026-10-12-w73-registry-schema-and-ledger-lifetime.md`
- [x] 6.3 W72 报告**追加勘误段**（A12–A14 的表述订正；遗留②已裁定）
- [x] 6.4 全仓检索「schema 校验」相关的不实陈述是否有第二处副本（C6 教训）

## 7 闭环

- [x] 7.1 跑 `cargo xtask check --profile full`
- [x] 7.2 全部 tasks 勾选后 `openspec archive -y w73-registry-schema-and-ledger-lifetime`
- [x] 7.3 `openspec validate --all --strict` **与** `--all --strict --archived`
- [x] 7.4 清理 `.tmp-*` 脚手架；一次 commit + push（提交信息用 Write 落 UTF-8 文件）
- [x] 7.5 匿名复核 CI 两平台结论并按观测轮约定回填报告
