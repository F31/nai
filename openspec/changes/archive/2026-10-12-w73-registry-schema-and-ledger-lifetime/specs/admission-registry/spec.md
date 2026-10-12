# admission-registry 变更（w73）

## MODIFIED Requirements

### Requirement: Single machine-readable admission registry

仓库 SHALL 维护 `tests/registry.yaml` 作为唯一验收来源，列出全部验收项并带 `release_line`、所属契约（如适用）、自动化等级、状态与证据引用；`release_line` MUST 使用独立字段区分验收编号与发布线名（slice_0、r1、r2、r3、experimental、**capability_pack**）。schema 校验 SHALL 拒绝重复编号、未知 release_line/status、悬空映射与新增的旧 T/E/U 编号。**该 schema 校验 MUST 真的被执行**——注册表 MUST 与 `schemas/tests/registry.schema.json` 逐字段校对，MUST NOT 出现「注册表写得进去、schema 认不出来」的状态；未知顶层键与未知条目字段一律拒绝。

#### Scenario: Duplicate or stale identifier
- **WHEN** 注册表中出现重复验收 ID 或新增旧 T/E/U 编号
- **THEN** 注册表校验失败，不进入后续切片任务

#### Scenario: Missing release line
- **WHEN** 某验收项缺少合法 release_line 或线号与 V3.10 §16 裁决不符
- **THEN** 校验失败并给出可读原因

#### Scenario: The registry is written but never checked against its schema

- **WHEN** `tests/registry.yaml` 出现一个 schema 未声明的顶层键或条目字段（W72 新增 `baseline_deviations`
  时即为此形状），而没有任何执行体把它交给该 schema
- **THEN** 该状态 MUST 被视为不满足本条：形状契约 MUST 在工程门控内被执行，
  MUST NOT 以「schema 文件存在」当作「schema 生效」；修复时 MUST NOT 只改 schema 的说明文字

### Requirement: The frozen baseline and the registry reconcile, modulo named deviations

`tests/registry.yaml` SHALL 与**冻结基线**逐 id 对账：§48.5 的家族索引与 §57.3 的发布线正本
（后者含表格与计数行两处写法，二者 MUST 互相自洽）。在册 MUST 等于基线声明，
**除** `baseline_deviations` 中逐条具名登记的偏差——每条 MUST 给出 id、登记轮次与非空理由，
且 MUST 对应一处**真实**差异；基线收录后未清空 MUST 按**陈旧条目**失败。
**任一条目的 `since_baseline` MUST 等于当前基线版本**——跨版只有两种处置：收录，或显式结转
（改 `since_baseline` + 记 `carried_at` + 非空理由）。
基线文档 SHALL 取 `docs/` 中版本号最大者，且 MUST 与 `openspec/config.yaml` 声明的一致。
对账 MUST NOT 是空洞的：章节号被改、该节缺失、计数行与表格不符、缺版本入参，均 MUST 分别具名失败。

#### Scenario: The baseline lags behind the registry

- **WHEN** 注册表在 `r1` 下登记了 `B07`（或把它移到了 `r2`），而冻结基线 §57.3 的 R1 行仍不含它
- **THEN** 对账失败并**同时给出该 id 与两侧的线**（在册所在线 / 基线声明所在线），
  MUST NOT 只报「条数不符」——条数不符有一百种原因，id 才是可执行的线索

#### Scenario: A deviation is registered but never collected

- **WHEN** 某偏差已被下一轮升基线收录（基线声明与在册重新一致），而 `baseline_deviations` 仍保留该条目
- **THEN** 对账失败并**指名该条目**、要求清空——否则台账只增不减，最终无人再读

#### Scenario: A deviation entry does not match any real difference

- **WHEN** `baseline_deviations` 登记了 `S02`，而实际差异是 `B07`
- **THEN** 两条都 MUST 失败：真实差异未被解释，且该条目**张冠李戴**，MUST NOT 因为「台账非空」而通过

#### Scenario: The section the gate reads is renamed or renumbered

- **WHEN** §48.5 或 §57.3 的**章节号**被改动，或该节被删除（改标题措辞**不**属此列）
- **THEN** 对账 MUST 报出「该节找不到」这一**解析失败**，与「设计漂移」分开具名，
  MUST NOT 因为解析结果为空集而**通过**

#### Scenario: The baseline pointer is stale

- **WHEN** `docs/` 中版本号最大的基线文档与 `openspec/config.yaml` 声明的不是同一份
- **THEN** 对账失败并**同时给出两个文件名**，MUST NOT 静默地对着其中一份跑

#### Scenario: A registered deviation survives a baseline upgrade

- **WHEN** 台账里有一条 `since_baseline` 为上一版的条目，而当前基线已经换版（该偏差既未收录、
  也未被显式结转）
- **THEN** 对账失败并**同时给出该条目的 id 与两侧版本**，MUST NOT 因为「条目仍然相称」而通过；
  正确处置只有两种——收录（删条目），或结转（改 `since_baseline` + 记 `carried_at` + 非空理由）

#### Scenario: The reconciliation is asked to decide without knowing the version

- **WHEN** 调用对账时没有给出当前基线版本（或被给出一个非法版本串）
- **THEN** 对账 MUST 具名报出「无法判定台账是否跨版」，MUST NOT **静默跳过**该判据——
  一条会因为入参缺失而恒真的断言，比没有断言更坏

## ADDED Requirements

### Requirement: The registry's shape layer and semantic layer are defined once

注册表的**形状** SHALL 由 `schemas/tests/registry.schema.json` 定义，**语义**（家族完整性、能力包集合、
发布线归属、跨文件契约绑定）由 `scripts/contracts/*.mjs` 定义，两者 MUST 同时生效。
它们在**三处**重叠——id 家族 pattern、发布线枚举、状态枚举——每处 MUST 逐字一致；
不一致时校验 MUST 失败并**指名是哪一处**（哪一边、哪个常量）。重叠是允许的，漂移不是。

#### Scenario: The identifier family pattern drifts between the two layers

- **WHEN** 脚本的 id 正则与 schema 的 `acceptance.id.pattern`（以及映射段 targets 的 pattern）不再相同
  （例如脚本里家族白名单少了一个字母）
- **THEN** 校验失败并指明是 **id 家族 pattern** 这一处漂移，MUST NOT 因为「两边都能跑」而通过

#### Scenario: The release-line or status enumeration drifts between the two layers

- **WHEN** 发布线枚举（脚本的 `RELEASE_LINES` 与 schema 的 `release_lines` 键集合 / `release_line.enum`）
  或状态枚举（脚本的状态集合与 schema 的 `status.enum`）有一边被改动
- **THEN** 校验失败并指出是**发布线枚举**还是**状态枚举**漂移，MUST NOT 静默取其中一边为准
