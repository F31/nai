# Demo Package Readiness

## Purpose

把 §615「演示包」的**构成要求**与 §612「可用性目标」的**时间目标**落成在 portable 下可执行、可测试的判定：
演示包必须齐备其五要素——**授权的**演示形象、示例知识、示例场景、Provider 预设、一键连通测试与成本预估——
其中「授权的」是一条要求而非形容词，Provider 预设只携带凭据**引用**、不得内嵌明文密钥，
一键连通测试必须是**真实探针**（连通成功**不是**验收达标），成本预估的未知分项必须**说明原因**；
可用性目标则由一份**演练记录**判定（未演练是 `Blocked`，不是缺陷）。本能力只判定**一份声明的演示包与演练记录**，
不联网、不读文件、不测量耗时；真实形象内容、真实连通与人工演练由外部条件提供。

## ADDED Requirements

### Requirement: A demo package is judged only once it is declared

一份**未声明**的演示包（五要素与演练记录全缺）SHALL 审计为空并读作 `Blocked`——尚未交付不是**违规**。
一旦声明了任一部分，其余部分即成为**欠项**，MUST 逐条具名报出。

#### Scenario: Nothing declared

- **WHEN** 空描述符被审计
- **THEN** 审计为空，且 readiness 为 `Blocked`

#### Scenario: A half-declared package

- **WHEN** 一份演示包只声明了示例知识、其余要素缺失
- **THEN** 审计报出缺失的每一部分并分别具名，**不**报出已声明的那一部分

### Requirement: The demo avatar is licensed, not merely present

声明了演示包时，**授权的**演示形象 SHALL 存在（§615）。形象缺失 MUST 报出 `demo_avatar_missing`；
形象存在但**未给出授权引用** MUST 报出 `demo_avatar_without_license`。

#### Scenario: No avatar

- **WHEN** 演示包未提供形象
- **THEN** 审计报出 `demo_avatar_missing`

#### Scenario: An avatar without a licence reference

- **WHEN** 演示包提供了形象但没有授权引用
- **THEN** 审计报出 `demo_avatar_without_license` 并指名该形象

### Requirement: Knowledge, scenario, provider presets and cost estimate are each required

声明了演示包时，示例知识、示例场景、Provider 预设与成本预估 SHALL 各自齐备（§615）。
任一缺失 MUST 被逐条具名报出。

#### Scenario: A missing part

- **WHEN** 演示包缺示例知识、示例场景、Provider 预设或成本预估中的任一项
- **THEN** 审计分别报出 `demo_knowledge_missing` / `demo_scenario_missing` /
  `demo_provider_presets_missing` / `demo_cost_estimate_missing`

### Requirement: Provider presets carry credential references, never embedded secrets

Provider 预设 SHALL 以**引用**（`credential_ref`）指向凭据，MUST NOT 内嵌明文密钥，也 MUST NOT 既无引用又无密钥。
内嵌明文密钥 MUST 报出 `provider_preset_embeds_secret` 并指名该预设；
既无引用又无密钥 MUST 报出 `provider_preset_without_credential` 并指名该预设。

#### Scenario: A preset that embeds a secret

- **WHEN** 某个 Provider 预设内嵌了明文密钥
- **THEN** 审计报出 `provider_preset_embeds_secret` 并指名该预设

#### Scenario: A preset with no credential at all

- **WHEN** 某个 Provider 预设既没有凭据引用也没有内嵌密钥
- **THEN** 审计报出 `provider_preset_without_credential` 并指名该预设

### Requirement: The connectivity test is a live probe, and a smoke result is not acceptance

一键连通测试 SHALL 使用**真实探针**；以常量结果冒充探针 MUST 报出 `connectivity_probe_is_stub`。
连通成功 MUST NOT 被当作验收达标：把冒烟结果声明为验收 MUST 报出 `smoke_claims_acceptance`。

#### Scenario: A constant stub

- **WHEN** 连通测试的探针是常量而非真实调用
- **THEN** 审计报出 `connectivity_probe_is_stub`

#### Scenario: A smoke result claimed as acceptance

- **WHEN** 演示包把一次连通成功声明为验收达标
- **THEN** 审计报出 `smoke_claims_acceptance`

### Requirement: An unknown cost component names its reason

成本预估 SHALL 逐项区分已知与未知。声明为未知的分项 MUST 说明**原因**；
未说明原因 MUST 报出 `cost_unknown_without_reason` 并指名该分项。

#### Scenario: An unknown component with no reason

- **WHEN** 某分项被声明为未知但没有给出原因
- **THEN** 审计报出 `cost_unknown_without_reason` 并指名该分项

### Requirement: Availability goals are judged against a drill record, and an unrun drill is Blocked

§612 的两条时间目标（约 10 分钟首句演练、约 30 分钟真实输出验证）SHALL 由一份**演练记录**判定。
记录缺项 MUST 报出 `drill_record_incomplete`；任一时间戳早于安装就绪 MUST 报出 `drill_timeline_inconsistent`；
两项耗时任一超过目标 MUST 分别报出 `first_utterance_beyond_target` / `real_output_beyond_target`。
**无**演练记录 MUST 读作 `Blocked`，MUST NOT 被读作缺陷，也 MUST NOT 被读作就绪。

#### Scenario: No drill has been run

- **WHEN** 演示包齐备但没有任何演练记录
- **THEN** readiness 为 `Blocked`，且审计为空

#### Scenario: A drill beyond the target

- **WHEN** 演练记录的首句耗时超过 10 分钟
- **THEN** 审计报出 `first_utterance_beyond_target`

#### Scenario: An inconsistent timeline

- **WHEN** 演练记录的首句时间早于安装就绪时间
- **THEN** 审计报出 `drill_timeline_inconsistent` 并指名该里程碑

### Requirement: Readiness is three-state and Defective outranks the others

readiness SHALL 为 `Blocked | Defective | Ready`。审计非空 MUST 取 `Defective`，**先于**是否已演练的判断——
一份违反边界的声明 MUST NOT 因子项未演练而被读成 `Blocked`。

#### Scenario: Defective outranks Blocked

- **WHEN** 一份演示包存在越界（如预设内嵌密钥）且尚无演练记录
- **THEN** readiness 为 `Defective`，而不是 `Blocked`

### Requirement: The demo package audit is a pure function of the descriptor

判定 SHALL 是对描述符自身字段的**纯函数**，报出全部命中项，MUST NOT 联网、读文件或测量耗时。
每条标记 MUST 可被构造，且一组构造描述的审计**并集** MUST **恰为**已登记的标记集合。

#### Scenario: A clean package audits empty

- **WHEN** 一份齐备且诚实的演示包被审计
- **THEN** 审计为空

#### Scenario: No unregistered token

- **WHEN** 一组覆盖全部规则的构造描述被审计
- **THEN** 其标记并集恰为登记的标记集合，不出现未登记的额外标记
