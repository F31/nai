# admission-registry 变更（w72）

## MODIFIED Requirements

### Requirement: Registry covers every baseline acceptance family

注册表 SHALL 覆盖基线（`§16` 与 `§39.5`、`§48.5`、`§51.3` 与 `§57.3`；`§` 引用按**当前基线**解析——
版本号只在 `docs/design.md` 与最新基线 ADR 处声明）定义的**全部**验收家族——`S`/`R`/`A`/`B`/`O`/`P`/`M`/**`H`**/`X`。
基线把某编号体系段列为验收时，其条目 MUST 全部登记在 `tests/registry.yaml`；
**「尚未开始」MUST 以 `pending` 表达，MUST NOT 以之为由省略条目**——在册与测得是两个不同的陈述。
校验 SHALL 拒绝不属这些家族的编号，
并 SHALL 断言每个「基线已定义完整编号段」的家族**逐号在册**；
该短语的取值 MUST 从 §48.5 家族索引**逐行读出**，MUST NOT 取自写死的清单——
§48.5 声明几个连续段，就覆盖几个（当前 9 个）。
家族**在册**断言与发布线**分线**断言是两个不同的检查，MUST NOT 互相替代：前者证「条目没被省略」，
后者证「归属没被错线」。

#### Scenario: A baseline family is entirely absent

- **WHEN** 基线把某家族定义为验收编号体系的一部分（如开放平台与技能市场的 M01–M18，见 §37.5/§38.10；
  或智能体 Harness 与开放互操作的 H01–H18，见 §48.5），而 `tests/registry.yaml` 未登记其中任何条目
- **THEN** 注册表视为不完整，`cargo xtask check --registry` MUST 失败并指出缺失的编号

#### Scenario: A gap inside a registered family

- **WHEN** 某家族已在册但**缺号**（例如 18 条中少了 `H05`），或出现超出该家族定义范围的编号（例如 `H19`）
- **THEN** 校验失败并**指名**缺失或多出的编号，MUST NOT 因为「家族非空」而通过
- **AND** 断言 MUST 按家族**分别**计数与比号，MUST NOT 用「合计至少 36 条」这类**可互相掩盖**的口径；
  且 MUST NOT 是空洞的——MUST 以「人为删掉某个在册编号」的注册表验证它确实失败

#### Scenario: Not-yet-started is registered, not omitted

- **WHEN** 某验收家族尚无可交付实现（其对应工作项未动工），但该家族**在**当前发布线的退出条件内
- **THEN** 其条目 MUST 以 `pending` 登记在册，MUST NOT 省略条目，MUST NOT 把状态写成 `implemented`/`blocked`

#### Scenario: An identifier outside every family is rejected

- **WHEN** 注册表中出现不属于这些家族的编号（如 `B01a`）
- **THEN** 校验失败，不进入后续切片任务

#### Scenario: The judgement source is narrowed to the families named today

- **WHEN** 某实现把「基线已定义完整编号段」的家族理解成一份**写死的清单**（例如只覆盖 M 与 H），
  而 §48.5 的家族索引声明了更多连续编号段
- **THEN** 该实现 MUST 被视为不满足本条：判据来源 MUST 是 §48.5 的正文，MUST NOT 是清单里的示例。
  括号里的旧文「（当前为 M01–M18 与 H01–H18）」是**示例**而非枚举，此点已由 W72 裁定

## ADDED Requirements

### Requirement: The frozen baseline and the registry reconcile, modulo named deviations

`tests/registry.yaml` SHALL 与**冻结基线**逐 id 对账：§48.5 的家族索引与 §57.3 的发布线正本
（后者含表格与计数行两处写法，二者 MUST 互相自洽）。在册 MUST 等于基线声明，
**除** `baseline_deviations` 中逐条具名登记的偏差——每条 MUST 给出 id、登记轮次与非空理由，
且 MUST 对应一处**真实**差异；基线收录后未清空 MUST 按**陈旧条目**失败。
基线文档 SHALL 取 `docs/` 中版本号最大者，且 MUST 与 `openspec/config.yaml` 声明的一致。
对账 MUST NOT 是空洞的：章节号被改、该节缺失、计数行与表格不符，均 MUST 分别具名失败。

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
