## Purpose

把 §30.1「不得用播放固定 MP4 冒充已完成的音频驱动模型」、§1469 声明式契约与 S07「日志、诊断包、崩溃转储、仓库扫描均无明文密钥」的**判定口径**落成在 portable 下可执行、可检查的骨架：新增一家 OpenAI 兼容端点**只是配置**（不新增协议分支、不新增适配器），凭据**只经进程环境变量**提供、**git 会发布的内容零明文**（扫描域是「已跟踪 ＋ 未跟踪但未被忽略」，被忽略的本地 `.env` 不在域内，否则本机凭据将无法存在；新增文件在暂存之前即入域），凭据缺失时**必须 Blocked、绝不回退桩或固定素材**，**模型可用性是账户状态而不是能力声明**（同端点上某模型 200、某模型 403 时须逐模型如实记录，不得互相代替），证据**只含事实、连凭据前缀都不含**，端点的契约差异（如音色标识书写形式）**以实测记录为准而非按供应商文档假设**，且一次连通冒烟**不得**被当作 B01（100 题集达门槛）或 O01（约 10 分钟首句演练语音）的达标宣称。本能力只判定**一份接入声明与一次真实调用的证据是否合格**；它不挑选供应商、不保存凭据、不替账户充值，也不执行 B01/O01 的完整验收。

## ADDED Requirements

### Requirement: A new OpenAI-compatible endpoint is configuration, not a protocol

新增一家 OpenAI 兼容端点 MUST 只作为**配置**接入：填入 `ProviderProfile` 的 `provider_ref`、`endpoint`、`model`、`price_revision`、`currency`、`timeout`、`max_retries` 与 `protocol`。MUST NOT 为此新增 `Protocol` 枚举分支，也 MUST NOT 新增适配器——`Protocol::OpenAiCompatible` 已同时 `serves` 文本与语音，同一端点的两个能力共享同一条凭据与同一个 `endpoint`。`provider_ref` MUST 与既有端点不同，否则用量账本在替换后无法区分两家。

#### Scenario: Wiring a text endpoint adds a profile and nothing else
- **WHEN** 接入一家以 `/v1/chat/completions` 提供文本的 OpenAI 兼容网关
- **THEN** 只新增一份 `ProviderProfile`（`protocol: openai`），`Protocol` 枚举与适配器集合不变

#### Scenario: One credential serves both capabilities of one endpoint
- **WHEN** 同一 OpenAI 兼容端点同时提供文本与语音
- **THEN** 该端点的文本与语音经同一 `endpoint` 与同一凭据绑定，不复制第二份端点配置

### Requirement: Credentials come from the process environment, never from a published file

凭据 MUST 只经**进程环境变量**注入（`NAI_LLM_API_KEY`、`NAI_TTS_API_KEY` 等）。**git 会发布的内容**中 MUST NOT 出现明文凭据。`cargo xtask check --secrets` MUST 以「已跟踪文件 **加** 未跟踪但未被忽略的文件」（`git ls-files --cached --others --exclude-standard`）为扫描域并断言其为零命中。扫描域 MUST NOT 是工作区磁盘，也 MUST NOT 只取已暂存集合：**被忽略的本地文件（如 `.env`）不在扫描域**，否则本机凭据无法存在；而**新增但尚未暂存**的文件 MUST 仍在域内，否则检查只能在错误几乎铸成之后才发现。扫描 MUST 覆盖已知高熵凭据前缀，并 MUST NOT 声称覆盖任意形态的密钥。

#### Scenario: A published file carries a high-entropy credential
- **WHEN** git 会发布的任一文件含有前缀加足额长度的凭据串
- **THEN** `cargo xtask check --secrets` 失败，并列出文件名与行号

#### Scenario: A new file is scanned before it is staged
- **WHEN** 一个新增文件尚未被 `git add`
- **THEN** 它已在扫描域内，凭据在暂存之前即被拦下

#### Scenario: The ignored local credential file is not in the scan domain
- **WHEN** 被忽略的本地 `.env` 持有真实凭据，而待发布内容均无凭据
- **THEN** `cargo xtask check --secrets` 通过

#### Scenario: A short sentinel is not mistaken for a credential
- **WHEN** 测试夹具含有同前缀但长度低于阈值的哨兵串
- **THEN** 扫描不命中

### Requirement: Missing credentials are Blocked, never a stub

任一所需凭据缺失时，真实链路冒烟 MUST 以 `CAPABILITY_NOT_READY` **非零退出**，并 MUST NOT 退回 `RehearsalTextGenerator`、`RehearsalSpeechSynthesizer` 或任何桩，也 MUST NOT 产出任何音频或生成结果文件。以桩的产出充当真实链路证据 MUST 被判为不合格（§30.1）。

#### Scenario: An absent credential stops the run
- **WHEN** 未设置语音端点的凭据
- **THEN** 冒烟以非零状态退出、报告 `CAPABILITY_NOT_READY`，且不写出音频文件

#### Scenario: A rehearsal stub cannot satisfy live evidence
- **WHEN** 一次运行的结果来自桩合成器而非端点响应
- **THEN** 该次运行不构成真实链路证据

### Requirement: Model availability is account state, not a capability claim

同一端点上 MUST 逐模型记录 HTTP 状态与网关错误码。某模型的 2xx MUST NOT 被用来宣称该端点**整体**可用；某模型的 403 或 503 MUST NOT 被用来宣称该端点不可用。网关给出的配额事实（含剩余余量）与渠道事实（如无可用通道）MUST 如实记录，且结论 MUST 只覆盖**被实测的那几个模型**。

#### Scenario: One model answers and another is refused by quota
- **WHEN** 同一端点上模型甲返回 2xx 而模型乙返回 403 且报文给出剩余余量
- **THEN** 证据逐模型分别记录状态、错误码与余量，且结论不推广到未实测的模型

#### Scenario: A missing channel is recorded as a channel fact
- **WHEN** 网关返回渠道缺失类错误
- **THEN** 证据记为该模型在该分组下无可用通道，不记作凭据无效

### Requirement: Evidence contains facts and carries no credential

证据 MUST 记录：端点主机名、模型标识、协议名、凭据**方案名**（如 `Bearer`）、HTTP 状态码、网关错误码、用量字段名与数值、响应字节数与 media type。证据 MUST NOT 含凭据本身——**连前缀也不得出现**——MUST NOT 含完整请求头值，MUST NOT 含可还原凭据的片段。

#### Scenario: The evidence file is free of credential material
- **WHEN** 一次真实调用成功并写出证据
- **THEN** 证据文件中不出现任何凭据前缀，授权信息只以方案名形式存在

#### Scenario: The request line is not reproduced verbatim
- **WHEN** 证据记录一次请求
- **THEN** 只记录主机、路径、方法与方案名，不记录 `Authorization` 头值

### Requirement: Audio evidence is real bytes

语音冒烟 MUST 落**真实音频字节**并记录字节数与 media type。空文件、桩音频或文本占位 MUST 被判为**未产出证据**。字节数与 media type MUST 来自响应本身，MUST NOT 由本地推断填写。

#### Scenario: Empty audio is not evidence
- **WHEN** 语音响应体为空或仅含错误文本
- **THEN** 该次运行不构成音频证据

#### Scenario: Real audio is evidence with measured facts
- **WHEN** 语音响应为非空音频且 media type 为音频类型
- **THEN** 证据记录实测字节数与 media type

### Requirement: Endpoint contract differences are recorded, not assumed

端点对同一语义的书写差异（例如音色标识的完整形式）MUST 以**实测记录**为准进入证据，MUST NOT 依据供应商文档假设，也 MUST NOT 在冒烟里静默归一化后只记结果。端点返回的 4xx MUST 连同其错误码如实记录；一个 4xx MUST NOT 被读作「端点不可用」，MUST NOT 被吞掉后改用默认取值重试。

#### Scenario: A bare voice name is refused with an explicit code
- **WHEN** 端点以显式错误码拒绝一个不合书写形式的音色标识
- **THEN** 证据如实记录该状态码与错误码，据此修正书写形式

#### Scenario: The accepted form is recorded as measured
- **WHEN** 按端点要求书写后返回音频
- **THEN** 证据记录该书写形式与其对应的状态码

### Requirement: A live smoke is connectivity evidence, not acceptance

一次连通冒烟 MUST NOT 被用来把 B01（100 题集达到质量门槛）或 O01（约 10 分钟首句演练语音、约 30 分钟完成真实输出验证）标为达标。这两项的达标 MUST 仍以各自的完整验收为准；冒烟成功 MUST NOT 改变它们的登记状态。

#### Scenario: A successful smoke leaves acceptance untouched
- **WHEN** 冒烟对两个端点均成功
- **THEN** B01 与 O01 的验收状态不变，仅新增连通性事实

#### Scenario: A blocked acceptance item stays blocked
- **WHEN** 真实模型已可用但其验收所需的题集或演练尚未完成
- **THEN** 该项保持其原有状态
