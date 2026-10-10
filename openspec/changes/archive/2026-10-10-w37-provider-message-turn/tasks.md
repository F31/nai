## 1. 规格与设计

- [x] 1.1 建立 OpenSpec 变更 `w37-provider-message-turn`，新增 `api-provider-transport` 差量需求（1 条需求 / 2 个场景），`validate --strict` 通过。
- [x] 1.2 落设计决策 D1–D9（落点、为何 user、否决 system+user、否决角色可配、不自拟提示文本、不改共享传输、测试如何钉两个方向、登记状态不变、推理模型余量）。

## 2. 适配器修正

- [x] 2.1 `crates/nai-provider-core/src/adapters/openai.rs`：`messages[0].role` 由 `system` 改为 `user`，并补注释说明「permitted context 是那唯一一条 user 轮，适配器不自拟提示文本」。
- [x] 2.2 确认 Anthropic 适配器**不动**（本就是 `user`），共享传输、协议枚举、契约、用量映射与预算接线**全部不变**。
- [x] 2.3 `scripts/provider-smoke.mjs` 与 `examples/live_provider_smoke.rs`：`max_tokens`/`max_output_tokens` 由 32 提到 256，附注释说明「推理模型需给可见输出留余量，否则 `finish_reason: length` 且可见输出为空 ⇒ 空心成功」。

## 3. 测试与文档

- [x] 3.1 `crates/nai-provider-core/tests/substitution.rs`：更新注释与断言——差异仍断言（路径、`stream_options`、凭据头），角色断言改为两个适配器都断 `user`。
- [x] 3.2 `docs/provider.md`：对照表「请求体」一行由 `messages[system] + stream_options` 改为 `messages[user] + stream_options`。
- [x] 3.3 `docs/reports/2026-10-10-w19-provider-substitution.md`：加**勘误**注记——表中角色一行是当时实现的写照、并非供应商差异，已由 W37 修正。
- [x] 3.4 `docs/reports/2026-10-10-w36-live-provider-evidence.md`：加**勘误**注记——该轮证据基于 `agnes-3.0-flash` 与当时的 `system` 角色适配器；W37 修正角色并改用 `agnes-2.5-flash`。
- [x] 3.5 `cargo test -p nai-provider-core --offline` 全绿（含 `substitution` 套件）。

## 4. 真实链路复测与证据

- [x] 4.1 以 `agnes-2.5-flash` 重跑 `cargo run -p nai-provider-core --example live_provider_smoke`：文本链路由 400 变为成功，重新生成 `docs/reports/evidence/live-provider-bound-stack.json`（文本 + 语音两条事实）。
- [x] 4.2 复跑 `node --env-file=.env scripts/provider-smoke.mjs` 与 `cargo xtask check --secrets`：证据零命中所属检查仍通过。

## 5. registry、归档与提交

- [x] 5.1 `tests/registry.yaml`：A02 notes 追加「W19 记录的差异中『角色』一项并非供应商差异而是不一致，已由 W37 修正为两种协议一致的用户轮」；B01/O01 状态**不变**。
- [x] 5.2 `docs/development-plan.md` 与 `AGENTS.md` 记录本轮。
- [x] 5.3 顺序纪律：`check --secrets` → `check --registry` → `check-deps` → `cargo test -p nai-provider-core --offline` → portable 全量门控 → 前端 typecheck/build → 勾 tasks → `openspec archive -y` → `validate --all --strict --archived` 全通过。
- [x] 5.4 一次 commit + push；commit 前确认 `.env` 未被跟踪、证据文件不含凭据。

## 里程碑切分

- **M1（§1–§3）**：适配器与规约一致，单测全绿。
- **M2（§4–§5）**：真实链路复测、registry 与闭环。
