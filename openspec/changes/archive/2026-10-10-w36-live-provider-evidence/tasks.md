## 1. 立项与骨架

- [x] 1.1 建立 OpenSpec 变更 `w36-live-provider-evidence`，新增 capability `live-provider-evidence` 差分规格（8 条需求），`validate --strict` 通过。
- [x] 1.2 落设计决策 D1–D12（落点、扫描域、阈值表、自命中规避、Blocked 纪律、账户状态、证据字段、音频真值、契约实测、主机差异、非验收、零改动 provider-core）。
- [x] 1.3 确认 18 包与依赖方向不变、零新增外部依赖（本变更只增 xtask 模块与 Node 脚本）。

## 2. 密钥扫描与凭据边界（S07）

- [x] 2.1 新增 `xtask/src/secrets.rs`：以 `git ls-files --cached --others --exclude-standard`（**git 会发布的内容**——不是磁盘，也不只取暂存集合）为扫描域，逐个读文本文件，按前缀表（`sk-`/`wk-`/`pk-`/`rk-` 32、`ghp_`/`github_pat_` 30、`glpat-` 18、`xox[baprs]-` 20、`AKIA` 16、`AIza` 30、`Bearer` 32）匹配；命中即收集（文件、行号）并失败。
- [x] 2.2 扫描器源码自身 MUST NOT 含可命中的字面量；测试夹具在运行期用 `format!` 拼装。
- [x] 2.3 挂 `lib.rs` 命令分发 `check --secrets` 与 `help` 文案。
- [x] 2.4 `--lib` 单测两个方向：被忽略的 `.env` **不在**域内，而**运行期新建、尚未暂存**的探针文件**在**域内；另有合成凭据命中并列出文件与行号、短哨兵不命中、扫描器自身源码不自命中。
- [x] 2.5 新增 `.env.example`（被跟踪）：只登记变量名与占位值（`NAI_LLM_BASE_URL`/`NAI_LLM_API_KEY`/`NAI_LLM_MODEL`/`NAI_TTS_BASE_URL`/`NAI_TTS_API_KEY`/`NAI_TTS_MODEL`/`NAI_TTS_VOICE`）。
- [x] 2.6 实测 `cargo xtask check --secrets` 在全部跟踪文件上零命中。

## 3. 真实链路冒烟与证据（B01 / O01 / A02）

- [x] 3.1 新增 `scripts/provider-smoke.mjs`（Node 22、零新增依赖、`--env-file` 读环境）：对文本端点发一次最小生成，对语音端点发一次短句合成。
- [x] 3.2 缺任一变量时以 `CAPABILITY_NOT_READY` 非零退出，打印缺失的**变量名**（不打印值），且**不写出**任何证据或音频；MUST NOT 回退 `RehearsalTextGenerator`/`RehearsalSpeechSynthesizer`。
- [x] 3.3 证据只含事实字段（`host`/`path`/`method`/`protocol`/`auth_scheme`/`model`/`status`/`gateway_error_code`/`usage_fields`/`bytes`/`media_type`/`note`），**不含凭据前缀、不含请求头原值**。
- [x] 3.4 音频落 `docs/reports/evidence/`，字节数取自响应体长度、media type 取自响应头；空体或错误报文判为未产出证据。
- [x] 3.5 实测运行：产出真实文本生成事实与真实音频字节；对证据文件跑密钥扫描确认零命中。

## 4. 端点契约与账户状态事实

- [x] 4.1 逐模型记录 HTTP 状态与网关错误码；配额报文提取剩余余量数值；结论只覆盖被实测的模型。
- [x] 4.2 记录端点真实模型清单与配置里模型名的差异（配置名不在清单内时如实记录）。
- [x] 4.3 记录音色标识的实际书写形式与对应状态码；**不做**静默归一化或失败重试掩饰。
- [x] 4.4 记录本机证书吊销检查（`CRYPT_E_REVOCATION_OFFLINE`）为**主机环境**注记，不得读作端点不可用。

## 5. 文档、registry、归档与提交

- [x] 5.1 新增 `docs/provider-live.md`（接入纪律、变量表、证据字段、边界与不覆盖项）。
- [x] 5.2 新增 `docs/adr/0038-live-provider-evidence-boundary.md`。
- [x] 5.3 新增 `docs/reports/2026-10-10-w36-live-provider-evidence.md`（实测事实表 + 门控结果）。
- [x] 5.4 `tests/registry.yaml`：S07 记录密钥扫描已可执行；O01 记录真实语音已产出真值音频（10 分钟首句演练仍待人工）；B01 记录真实模型已可用但题集未定，**状态保持 `blocked`**；A02 记录真实第二家链路已实测。
- [x] 5.5 `docs/development-plan.md` 与 `AGENTS.md` 记录本轮。
- [x] 5.6 按顺序纪律：跑 `check --secrets`、`check --registry`、`check-deps`、portable 全量门控、前端 typecheck/build 与冒烟 → 勾 tasks → `openspec archive -y` → `validate --all --strict --archived` 全通过。
- [x] 5.7 一次 commit + push；commit 前确认 `.env` **未被跟踪**且证据文件不含凭据。

## 里程碑切分

- **M1（§1–§2）**：扫描器与凭据边界可执行、零误报；这是让「拿到真实密钥」这件事**安全**的前提。
- **M2（§3–§4）**：真实链路事实与音频证据落盘；这是让 B01/O01/A02 **有真值可依**的前提。
- **M3（§5）**：文档、registry 与闭环。
