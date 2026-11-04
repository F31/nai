## ADDED Requirements

### Requirement: An in-repository manual demo package ships valid licensed 2D assets and a scene

`demos/manual-demo/` SHALL 是随源码入仓的人工辅助演示包：包含 schema 合法的 operator 场景
（provider 预设/知识/prompt/预算/render 闭包）、三张真实 RGBA 素材（manifest 中 SHA-256 逐一锁定）、
许可引用（`nai-demo-license` 指向随包的 LICENSE 文件）、成本预估与连通性声明。素材 MUST 通过
既有 2D 准入（`software-rgba8-v1`：精确三资产闭包、每张素材的 SHA-256 与许可引用、图层与尺寸
schema 合法）；许可引用 MUST 真实存在落点，MUST NOT 冒充第三方形象、品牌或重量级素材授权。

#### Scenario: The shipped package admits as a 2D closure

- **WHEN** engine 对 `demos/manual-demo/avatar/manifest.json` 做素材准入
- **THEN** 三张素材 SHA-256 与许可均通过，`licenses: ["nai-demo-license"]` 成立，且 manifest.config
  通过 `schemas/plugin/avatar` 与 `schemas/config/avatar-2d`

#### Scenario: The shipped scene is a valid operator scene

- **WHEN** `demos/manual-demo/manual-session.json` 被 operator 场景 schema 校验
- **THEN** provider/knowledge/prompt/budget/render 全部 schema 合法，render 段引用上述素材

### Requirement: A one-key preflight reports per-axis verdicts and refuses on uncertainty

`nai-engine --preflight <场景>` SHALL 只读地检查并逐轴报告 `passed`/`failed`/`not_ready`：场景 schema、
知识文档唯一且非空、provider 预设可构造（`credential_env` 已设置）、预算字段在 schema 界内、
render 闭包素材准入、成本预估存在且「未知」附原因、连通性声明一致。它 **MUST NOT** provisioning、
MUST NOT 启动会话或服务，MUST NOT 在无凭据时发起 Provider 调用；任一轴 `failed` 或 `not_ready`
SHALL 使整体拒绝（退出码非 0），并逐轴给原因。预检 MUST NOT 声称已播放、已接设备或已生产
PlaybackAck。

#### Scenario: A legal package passes without a running session

- **WHEN** 对合法演示包运行 `--preflight`，且 `NAI_DEMO_OPERATOR_KEY` 已设置、Provider 端点可达
  （受预算约束的探针）
- **THEN** 各轴为 `passed`，整体放行；不 provisioning、不启动服务

#### Scenario: Missing credential is not ready, not passed

- **WHEN** `credential_env` 未设置时的预检
- **THEN** provider 轴报 `not_ready` 并点名缺失环境变量名，整体拒绝

#### Scenario: A corrupted asset is failed

- **WHEN** manifest 中 SHA-256 与素材实际字节不符
- **THEN** render 轴报 `failed` 并指名素材与算法，整体拒绝

#### Scenario: No render closure is not ready

- **WHEN** 场景无 `render` 段时的预检
- **THEN** render 轴报 `not_ready` 并说明演示包需要 2D 闭包才可演练，整体拒绝

### Requirement: The preflight can run a bounded connectivity probe or decline

预检 SHALL 支持在有凭据且显式声明连通测试时的**受预算约束的真实探针**；探针结果 MUST NOT 被
记录为验收或来源 eligibility。无凭据或未声明连通时 SHALL 报 `not_ready` 并说明探针未发，MUST NOT
以跳过来冒充通过。

#### Scenario: Probe with a configured credential

- **WHEN** 凭据已设置且显式选择探针
- **THEN** 发起一条受预算约束的真实 Provider 请求并对每个被测轴分别报告结果，不重复计费、不重发

#### Scenario: Declined probe

- **WHEN** 未设置凭据或未选择探针
- **THEN** 报 `not_ready`（探针未发），不发出任何 Provider 调用