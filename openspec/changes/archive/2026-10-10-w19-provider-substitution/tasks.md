## 1. 传输接缝去硬编码

- [x] 1.1 `client.rs` 新增 `AuthScheme`（`Bearer` 默认 | `Header{name}`）、`extra_headers`、`provider_ref`；`request()` 按 scheme 构造凭据头，共享路径对所有供应商相同。
- [x] 1.2 严格校验：头名须为 RFC 9110 token 且不属传输自管头（`authorization`/`host`/`content-length`/`content-type`）、不得重复、值不得含 CR/LF/NUL、条数/长度有界；`provider_ref` 非空且有界。
- [x] 1.3 `Usage.provider_ref` 由 endpoint 改为稳定供应商身份（`adapters/openai.rs`）。
- [x] 1.4 更新全部构造点：`tests/provider.rs`、`examples/openai_smoke.rs`；`lib.rs` 导出 `AuthScheme`。

## 2. 第二家 LLM 协议

- [x] 2.1 新增 `adapters/anthropic.rs`：`POST /messages`、`x-api-key`、`anthropic-version`、请求体 `system`/`messages`、SSE 按 JSON `type` 分派、`content_block_delta` 取文本。
- [x] 2.2 用量 `input_tokens`/`output_tokens` 在适配器边界映射为 `prompt_tokens`/`completion_tokens`/`total_tokens`，零字段省略；封闭词表不放宽（含单测）。
- [x] 2.3 流错误、字节上界、取消/期限与既有适配器同构。

## 3. 第二家 TTS 协议与绑定接缝

- [x] 3.1 新增 `adapters/elevenlabs.rs`：`POST /v1/text-to-speech/{voice}?output_format=pcm_{rate}`、`xi-api-key`、体 `text`/`model_id`、裸二进制 PCM。
- [x] 3.2 voice 身份作路径段先校验（不得改变路径形状）；供应商命名不了的采样率与非单声道**显式拒绝**，并在拒绝时释放预留。
- [x] 3.3 字符成本优先用响应头 `x-character-count`，缺失时用发出文本的确定性字符数（含两条单测）。
- [x] 3.4 新增 `binding.rs`：`Protocol`（含 `from_config`/`as_config`/`capability`/派生 auth 与静态头）、`ProviderProfile`（不含凭据）、`LlmClient`/`SpeechClient` 能力切面；能力不符**显式报错**。
- [x] 3.5 `lib.rs` 导出接缝类型。

## 4. 替换验证与全量校验

- [x] 4.1 新增 `tests/substitution.rs`：两台本地 mock（无外网），同一驱动函数 `generate_once` / `speak_once`。
- [x] 4.2 断言线上差异确实存在（路径、凭据头名、请求体形状、voice 在体内还是路径内）——防「同一协议跑两遍」的假通过。
- [x] 4.3 断言调用方可见结果形状一致；两家用量字段名都落进同一封闭单位表；不报用量的供应商**不被编造**实际值。
- [x] 4.4 断言语音按句独立（两次调用均从 `seq=0` 起、`sample_start` 恒 0、各自结算 usage）与不可达采样率的拒绝。
- [x] 4.5 `cargo test -p nai-provider-core` 全绿；clippy `-D warnings` 干净。

## 5. 规格、文档与提交

> 勘误（2026-10-10）：本节 5.2–5.6 曾在本轮实施过程中被预先勾选，但经核实磁盘上并无对应产物
> （`docs/provider.md` 未改动、`docs/adr/0025-*.md` 与报告缺失、`tests/registry.yaml` / `AGENTS.md` /
> `docs/development-plan.md` 无本轮记录、变更未归档、代码未提交）。现按真实状态回调，逐项落实后再勾选；
> 不保留「未做却已勾」的状态。

- [x] 5.1 OpenSpec 变更 `w19-provider-substitution`（proposal/design/tasks/`api-provider-transport` 增量）编写完成，`validate --strict` 通过。
- [x] 5.2 更新 `docs/provider.md`（传输接缝 `AuthScheme`/`extra_headers`/`provider_ref`、三家协议对照、`binding.rs` 配置驱动替换、`substitution.rs` 验证要点、按句边界、未编造用量）。
- [x] 5.3 新增 ADR 0025 记录替换单位的选择与三条否决方案。
- [x] 5.4 报告 `docs/reports/2026-10-10-w19-provider-substitution.md`：实测数字、线上差异证据、未覆盖边界。
- [x] 5.5 `docs/development-plan.md` 与 `AGENTS.md` 记录本轮；`tests/registry.yaml` 补 A02/B03 证据且**不越权升级**产品项状态。
- [x] 5.6 归档 `w19-provider-substitution`，跑 `validate --all --strict`（含 `--archived`）。
- [ ] 5.7 全量 `cargo xtask check --profile portable`（`CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=4`）通过并留档；提交 + 推送。
