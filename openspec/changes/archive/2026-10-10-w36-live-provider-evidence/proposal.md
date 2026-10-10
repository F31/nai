## Why

B01、O01、A02 与 B03 此前一律登记为「真实模型未接」：判定骨架都已落地（W30/W31 的黄金集门槛与载荷载体、W19 的第二家替换、W29 的度量与复盘），但**没有任何一次真实调用**发生过。真实凭据到位的当下，缺的不是机制而是**接入纪律**——而这正是本项目最容易翻车的地方：

- **凭据会顺手进仓库**。本机刚拿到两把真实密钥；`git add -A` 与临时提交信息文件的历史教训（W32/W33/W34/W35 连续四轮）说明「顺手」是常态。S07 的 notes 至今写着「CI 密钥扫描待建」。
- **缺少凭据时最容易假装连通**。桩合成器与回放路径都在，静默回退能让冒烟「绿」，而这恰是 §30.1 点名的禁止项。
- **端点可用性极易被过度概括**。同一网关上模型甲 2xx、模型乙 403（余额为负）是**账户状态**，不是「这家不能用」；反过来也不能因为 flash 通了就说整个网关可用。
- **供应商文档与实际契约会不一致**。音色标识的实际书写形式与直觉不同，若不按实测记录、而是按文档假设并静默归一化，证据就失去了纠错能力。

这四项合起来构成一条**接入纪律**，它可以在 portable 下被机器检查，而不是写进文档靠自觉。

## What Changes

- **新增能力 `live-provider-evidence`**：判定一份真实端点接入声明与一次真实调用证据是否合格（8 条需求）。
- **新增 xtask 检查 `check --secrets`**（`xtask/src/secrets.rs`）：扫描域取 `git ls-files --cached --others --exclude-standard`——**git 会发布的内容**，既不是磁盘（那会让本机凭据必然自命中而使检查被绕过），也不只是暂存集合（那会让新增文件在 `git add` 之前逃过检查）。按「前缀 + 各前缀最小长度」表扫描高熵凭据，命中即列出文件与行号并失败。这条检查补上 S07 登记中「CI 密钥扫描待建」的缺口。
- **新增 `.env.example`**（被跟踪）：只登记变量名与占位值，不含任何真实凭据。
- **新增 `scripts/provider-smoke.mjs`**（Node 22，零新增依赖，沿用 `control-smoke.mjs`/`engine-smoke.mjs` 风格）：经 `--env-file` 读环境，对文本与语音两个真实端点各发一次真实请求，写出**只含事实**的证据（端点主机、模型、协议名、方案名、HTTP 状态、网关错误码、用量字段、字节数、media type）与真实音频字节；任一凭据缺失即以 `CAPABILITY_NOT_READY` 非零退出。真实音频落 `docs/reports/evidence/`。
- **不改动 `crates/nai-provider-core`**：两家端点都是 OpenAI 兼容，`Protocol::OpenAiCompatible` 已同时 `serves` 文本与语音，接入只是配置。本变更**零新增外部依赖**、18 包与依赖方向不变。
- **registry**：S07 记录密钥扫描已可执行；O01 记录真实语音已产出真值音频（10 分钟首句演练仍需人工）；B01 记录真实模型已可用但题集未定，**状态保持 `blocked`**；A02 记录真实第二家链路已实测。

## Capabilities

### New Capabilities

- `live-provider-evidence`: 真实 Provider 接入纪律与真实调用证据的判定——端点接入是配置而非协议、凭据只经环境变量且跟踪文件零明文、缺凭据必须 Blocked 不得回退桩、模型可用性是账户状态而非能力声明、证据只含事实不含凭据、音频证据必须是真实字节、端点契约差异以实测为准、连通冒烟不得冒充 B01/O01 的达标。

### Modified Capabilities

无。本变更不修改任何既有能力规格；`nai-provider-core` 的 `Protocol`/`ProviderProfile`/`LlmClient`/`SpeechClient` 契约不变。

## Impact

- 新增 `xtask/src/secrets.rs`、挂载 `xtask/src/lib.rs` 的命令分发与 `help` 文案；新增 `--lib` 单测。
- 新增 `scripts/provider-smoke.mjs`、`.env.example`、`docs/reports/evidence/`（证据与音频）。
- 新增 `docs/provider-live.md`、`docs/adr/0038-live-provider-evidence-boundary.md`、`docs/reports/2026-10-10-w36-live-provider-evidence.md`。
- 更新 `tests/registry.yaml`（S07 / O01 / B01 / A02 的 notes）、`docs/development-plan.md`、`AGENTS.md`。
- **本地新增未跟踪 `.env`**（持真实凭据，`.gitignore` 第 4 行已覆盖）：MUST NOT 被提交。
- 主机差异：语音端点在本机需关闭证书吊销检查（`CRYPT_E_REVOCATION_OFFLINE`），这是本机 schannel 环境事实，不是端点事实；证据 MUST 记录该事实而不得把它读作端点不可用。

## Non-Goals

- 不挑选或推荐供应商，不替账户充值；**余额不足是外部事实**，不由本变更"解决"。
- 不把一次连通冒烟当作 B01/O01 的达标；100 题集内容与 10 分钟首句演练仍按各自验收。
- 不实现密钥的加密存储、轮换或密钥管理服务；本变更只保证**跟踪文件零明文**与**缺凭据即 Blocked**。
- 不覆盖任意形态的密钥检测（例如无已知前缀的自定义令牌）；扫描范围以已知高熵前缀为界，并在文档中**如实声明该边界**。
- 不改动 `nai-provider-core` 的协议枚举、适配器或契约。
