## 1. 规格与设计

- [x] 1.1 建立 OpenSpec 变更 `w40-golden-eval-runner`，新增 `knowledge-quality-gate` 差量需求（3 条需求 / 11 个场景），`validate --strict` 通过。
- [x] 1.2 落设计决策 D1–D10（参考答案必填、修订 1 不做双读、判定是纯函数且落 kernel、`evaluate` 签名不变、执行体落 xtask example、缺前置拒绝桩替代、检索侧不得伪造、零新增包、B01 保持 blocked、不代造题）。
- [x] 1.3 把缺口的四段性质写成据可核对的发现（题集无答案判据、无执行体、真实知识库属外部、题集内容属外部），各附文件行号。

## 2. kernel：参考答案判据

- [x] 2.1 `crates/nai-kernel/src/golden.rs` 新增 `ExpectedAnswer { must_include, must_exclude }` 与 `GoldenCase.expected`。
- [x] 2.2 新增纯函数 `judge_answer(&ExpectedAnswer, &str) -> AnswerVerdict`：归一化（小写折叠 + 空白折叠）后按「不得包含优先」判定。
- [x] 2.3 `crates/nai-kernel/tests/golden.rs` 补测试：全部命中为 Correct、缺一条为 Incorrect、`must_exclude` 命中覆盖全部命中、空白/大小写不改变结论。
- [x] 2.4 修正既有构造点（`GoldenCase` 新增字段）并确认既有 24 项断言语义不变。

## 3. 题集 schema 修订 2

- [x] 3.1 `xtask/src/golden.rs`：`CASE_SET_SCHEMA = 2`，`CASE_KEYS` 加入 `expected`。
- [x] 3.2 新增 `parse_expected`：`kind = reference`、`must_include` 至少一条、`must_exclude` 可缺省；未知 kind 与未知字段按既有 token 规则拒绝。
- [x] 3.3 把「`case_set_schema: 2` 被拒」那条测试改为「2 被接受，1 与 3 被拒」，并补 `expected` 缺失/空数组的缺陷测试。
- [x] 3.4 夹具 `tests/fixtures/rag-golden/example-min.json` 升到修订 2 并补 `expected`（仍如实标注不是真值）。

## 4. 评估执行体

- [x] 4.1 新增 `xtask/examples/golden_eval.rs`：读 `tests/rag-golden`（复用 `xtask::golden` 解析器）→ 真实 `TextGenerator` 逐题生成 → `judge_answer` → `evaluate` → 打印 `QualityReport`。
- [x] 4.2 退出状态分明：题集未提供/结构不合规/凭据缺失 ⇒ `CAPABILITY_NOT_READY`（1，且不发调用）；真实调用失败 ⇒ 2。
- [x] 4.3 凭据只从环境读取，输出的只有变量名；证据 JSON 写入 `docs/reports/evidence/` 且不含凭据。
- [x] 4.4 检索侧无真实知识库时记为**未测量**（具名），不填 0、不伪造 `retrieved`。
- [x] 4.5 `xtask/Cargo.toml` 的 `[dev-dependencies]` 加 `nai-provider-core`、`nai-storage`、`tokio`；确认 `check-deps` 仍 18 包、portable 覆盖范围不变。

## 5. 文档

- [x] 5.1 新增 `docs/adr/0040-golden-eval-runner-boundary.md`：执行体归属、参考答案判据形态、与 ADR 0033 的关系（补足而非推翻）。
- [x] 5.2 更新 `schemas/rag-golden/case-set.schema.json` 到修订 2（`expected` 必填、`must_include` 至少一条）。
- [x] 5.3 更新 `docs/golden-set.md` 与 `tests/rag-golden/README.md`：参考答案字段、执行体命令与退出码、未测量口径。
- [x] 5.4 更新 `tests/registry.yaml` B01 notes（保持 `blocked`，记录缺口从 4 段降到 2 段）与 `docs/development-plan.md`、`AGENTS.md`。

## 6. 门控闭环

- [x] 6.1 写 `docs/reports/2026-10-11-w40-golden-eval-runner.md`（发现、交付、实测、未覆盖边界）。
- [x] 6.2 顺序纪律：`check --secrets` → `check --registry` → `check-deps` → 前端 typecheck/build/desktop 测试/`test:ui` → `check --golden` → portable 全量门控 → 勾 tasks → `openspec archive -y` → `validate --all --strict --archived` 全通过。
- [x] 6.3 一次 commit + push；commit 前确认 `.env` 未被跟踪、无凭据入库。
