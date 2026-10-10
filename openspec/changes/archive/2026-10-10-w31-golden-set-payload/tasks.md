# W31 任务

## 1. 载荷规范与位置（§30.1、§5.9、§6）

- [x] 1.1 新增 `schemas/rag-golden/case-set.schema.json`：`case_set_schema`（const 1）、`scenario_version`、`cases`；`expectation` 五形（`evidence`/`high_risk_fact`/`must_clarify_or_escalate`/`must_not_act`/`must_cancel`）、`origin` 两形（`authored`/`promoted`+非空 `confirmer`）；`additionalProperties: false` 全面拒绝未知字段；`cases` 的 §6 最低 100 条写在 `description` 中并由门禁具名报出（schema 自身只要求非空，以便缺陷能被**报告**而不是只能被拒绝）。
- [x] 1.2 新增 `tests/rag-golden/README.md`：声明这是 §30.1 指定的唯一位置；列出题集的**三条来源**（按场景人工编写 / 随场景包发布 / 坏案例经具名人工确认后回流）与产出方；给出格式与两条校验命令；给出三种结论的对照表；写明当前状态为**未提供**。

## 2. 载荷读取与就位门禁（`xtask/src/golden.rs`）

- [x] 2.1 `Defect { file, token, detail }` 与 kernel 的 `Finding` 分开：缺陷＝文件读不出用例集，finding＝读出来了但没达到结构规则。
- [x] 2.2 `PayloadAudit { files, sets, defects }`；`is_unsupplied()` 以 `files == 0` 为唯一判据（MUST NOT 由 `sets.is_empty()` 推断，否则「有文件但全被拒」会被误报成「未提供」）；`passed()` 要求非未提供、无缺陷、无 finding。
- [x] 2.3 `Report` 收集器把文件名附着到每条缺陷；`keys()` 拒绝未知字段而非忽略；`text()` 把空白视为缺失（`confirmer: "  "` 与缺失同罚）。
- [x] 2.4 `audit_case_set(file, source)`：JSON 解析失败 → `json_invalid`；`case_set_schema` 非 1 → `case_set_schema_unsupported`（含实际值与期望值）；逐用例解析（`category_unknown` / `expectation_kind_unknown` / `origin_kind_unknown` / `field_missing` / `field_empty` / `field_type` / `unknown_field`，全部带 JSON 路径）；再构造 `GoldenSet` 逐条 `add`（重复 id → `duplicate_case_id`）；最后取 kernel 的 `audit_coverage()` 作为 findings。**任何阶段有缺陷即返回全部缺陷，不做部分计分。**
- [x] 2.5 `audit_directory(dir)`：只读直接子级的 `.json`，按文件名排序；目录不存在返回空审计（由 `check_dir` 转成明确拒绝）。
- [x] 2.6 `check(root, dir)` / `check_dir(path, label)`：打印 `files`、每个题集的场景版本与用例数、每条 finding 与缺陷；`files == 0` ⇒ `CAPABILITY_NOT_READY` 并指出被搜索目录；有缺陷 ⇒ 报缺陷数；有 finding ⇒ 报 finding 数；否则报通过。
- [x] 2.7 `xtask/src/lib.rs`：新命令 `check --golden` 与 `check --golden --dir <路径>`，更新 help 文案。
- [x] 2.8 `xtask/Cargo.toml`：新增 `nai-kernel.workspace = true`（tool 层允许；外部依赖零新增）。

## 3. 测试（`xtask/src/golden.rs` 内 `#[cfg(test)]`，随 portable 门控的 `--package nai-xtask --lib` 一起跑）

- [x] 3.1 结构：短集具名报 `below_minimum_cases`（`8 of 100`）；100 题覆盖八类无 finding；100 题全同类 ⇒ 恰好 7 条 `category_uncovered`。
- [x] 3.2 期望与类别：不兼容期望报 `expectation_category_mismatch` 并指向用例 id；空 `required` 与空 `forbidden_claims` 都在读入时被拒（缺陷归题集，不归系统）。
- [x] 3.3 修订与字段：非 1 的 `case_set_schema` 被拒并报出实际/期望值；未知顶层字段与空 `cases` 同时报出且带路径；`{` ⇒ 单条 `json_invalid`。
- [x] 3.4 词汇表：未知 `category` 与未知 `expectation.kind` 各自被拒。
- [x] 3.5 确认人：`promoted` 缺 `confirmer` ⇒ `field_missing`；`"  "` ⇒ `field_empty`；具名 ⇒ 读入成功。
- [x] 3.6 重复 id ⇒ `duplicate_case_id` 并报出该 id。
- [x] 3.7 目录：不存在的目录 ⇒ `unsupplied()` 且 `check_dir` 返回以 `CAPABILITY_NOT_READY` 开头、含目录名的错误；只读 `.json` 且按名排序（`notes.md` 被忽略）；100 题载荷 `passed()` 且 `check_dir` 通过，8 题载荷以「structural rules」结束；解析失败的文件报「structural defect」而**不是**「未就绪」或「题数不足」；修订不受支持经目录扫描也被拒。
- [x] 3.8 实测：`cargo test -p nai-xtask --lib` ⇒ **27 passed / 0 failed**（其中 golden 17 项、原有 deps 8 + schema 2）。

## 4. 夹具与实跑

- [x] 4.1 新增 `tests/fixtures/rag-golden/example-min.json`：构造的 8 例最小集（八类各一，其中一例为具名 `promoted`），**明确不是真值**，且不在默认位置故不被默认命令读取。
- [x] 4.2 实跑 `cargo xtask check --golden` ⇒ `files: 0` + `CAPABILITY_NOT_READY`（指出 `tests/rag-golden`），退出码 1。
- [x] 4.3 实跑 `cargo xtask check --golden --dir tests/fixtures/rag-golden` ⇒ 读入 8 例、报 `below_minimum_cases: 8 of 100`、以「structural rules」结束——证明加载器与判定可用且两种结论不混淆。
- [x] 4.4 `cargo xtask check-deps` ⇒ 18 包、依赖方向不变；`cargo clippy -p nai-xtask --all-targets --locked -- -D warnings` 零告警；`cargo fmt --all` 已跑。

## 5. 口径与文档

- [x] 5.1 `tests/registry.yaml`：B01 由 `implemented` **回调为 `blocked`**（§655 验收为「100 题集达到第 6 章门槛」＝实测达标，题集不存在时不可能达成）；notes 完整保留 W30 判定机制与 W31 载体的证据并写明阻塞原因。**不新增 B01a/B01b**（id 正则要求恰好两位数字，拆分会使 `check --registry` 失败，且等于改写规格口径）。
- [x] 5.2 `docs/golden-set.md` 增补「题集的来源与位置」一节：位置、三条来源、格式、三种结论、当前状态。
- [x] 5.3 新增 ADR 0033，记录落点（tool 层而非 kernel/shared）、格式（JSON 而非 YAML）、`files == 0` 作为「未提供」的唯一判据、确认人前移到载荷边界、以及七项否决方案。
- [x] 5.4 报告 `docs/reports/2026-10-10-w31-golden-set-payload.md`：实测数字、逐项验收映射、未覆盖边界。
- [x] 5.5 全量 `cargo xtask check --profile portable` 通过并留档；提交 + 推送。

## 里程碑切分

本轮为单个可独立验证的切片：位置与格式（§1）、读取与门禁（§2）、测试（§3）、夹具与实跑（§4）共用同一份 `knowledge-quality-gate` 增量，且 §3 的测试逐条断言的是 §2 的行为——拆开提交会让门禁的第一版无法证明「未提供」与「缺陷」确实被分开。文档、registry、ADR 与报告（§5）为纯声明变更，沿用同一次门控留档。

**不在本轮**：任何真实题集内容、真实模型评估、真实检索质量，以及「从日志自动生成题集」的路径（§5.9 要求人工确认）。
