# Spec delta: W137 — O05 的成本来源

## MODIFIED Requirements

### Requirement: Upgrade cost is listed per component, and an unknown component is not rendered as zero

桌面 SHALL 把升级成本**逐项列示**（云资源、下载、存储、预估运行增量）。无法估算的分项 MUST 呈现为
**未知及其原因**，MUST NOT 呈现为 0；总额若含未知分项，MUST 呈现为**下界加未知计数**，MUST NOT 呈现为总额。
载荷 MUST 被严格解析：缺字段、未知字段、非法金额或空原因 MUST 导致**具名拒绝**，不得静默猜补。

成本的**来源**由 `POST /api/v1/upgrades/plan` 提供（W137）：桌面端 MUST NOT 自行计算成本分项，
MUST 把包清单交给控制面并解析其返回的 `UpgradePlanView`。金额 MUST 与
「来自包清单声明、未验签」这条边界一同呈现——省略它就把「别人说多少钱」显示成了「要花多少钱」。

真实升级界面的渲染与生产接线仍**不在**本需求内，故它仍然不构成 O05 的完整 UI 验收。

#### Scenario: An unknown component

- **WHEN** 某分项被声明为未知并给出原因
- **THEN** 该行呈现为「未知（原因）」，且不出现数字 0

#### Scenario: A partially known total

- **WHEN** 部分分项已知、部分未知
- **THEN** 总额呈现为下界加未知计数，且明确**不是**总额

#### Scenario: A malformed payload

- **WHEN** 载荷缺少必需字段、含未知字段、金额不是非负安全整数或原因为空
- **THEN** 解析具名失败，呈现层不产出任何行

#### Scenario: The envelope is not a place to be generous

- **WHEN** 升级计划的响应里多出一个契约未声明的字段（例如服务端自作主张补一个 `total`）
- **THEN** 解析具名拒绝，MUST NOT 忽略该字段继续呈现

## ADDED Requirements

### Requirement: The upgrade plan answers cost from the manifest, synchronously and from one definition

控制面 SHALL 提供 `POST /api/v1/upgrades/plan`（权限 `runtime.upgrade`），就请求里的包清单**同步**
给出四类成本。本路径 MUST NOT 下载、暂存或迁移，故 MUST NOT 铸造作业 id——一个查不到东西的
`job_id` 是伪造的可观测面。成本 MUST 由 `nai-upgrade-core` 的 `CostEstimate` 算（定义处只有一处），
控制面与前端 MUST NOT 各算一份。清单结构 MUST 由 `schemas/config/upgrade-package.schema.json`
校验，MUST NOT 在 OpenAPI 里重述其字段。响应 MUST 满足它自己声明的 schema。

#### Scenario: A component the manifest leaves out

- **WHEN** 清单声明 cloud 为 10 而 download/storage/runtime_delta 为 null
- **THEN** 三项各自报告为**未知及其原因**，MUST NOT 出现 `known: 0`；把未知写成 0 的实现不满足本需求

#### Scenario: An explicit zero

- **WHEN** 清单把某分项声明为 `"0"`
- **THEN** 该分项报告为已知的 0——0 是一个可被证伪的断言，必须如实呈现，MUST NOT 与「未知」混同

### Requirement: A manifest the plan cannot present is refused by name, never turned into zeros

无法呈现的清单 MUST 被**具名拒绝**，MUST NOT 用「全未知」或「全 0」的 200 代替：币种不是三字母大写码、
金额超过 `2^53-1`（消费端会悄悄取整）、清单不合规，三种各有其名，且拒绝响应里 MUST NOT 出现成本面。
这几种差异 MUST NOT 靠放宽既有呈现判据来迁就——方向取拒绝。

#### Scenario: A currency that cannot be presented

- **WHEN** 清单的 currency 是 `cny`（清单 schema 允许 1–8 个字符，呈现契约要求三字母大写）
- **THEN** 具名拒绝，且响应里没有成本面；MUST NOT 放宽呈现口径去接受它

#### Scenario: An amount past the representable range

- **WHEN** 某分项声明为 9007199254740992（2^53）
- **THEN** 具名拒绝；MUST NOT 返回一个自己契约（`maximum: 2^53-1`）不合格的 200

#### Scenario: A manifest that fails its own schema

- **WHEN** 清单缺少 `cost.runtime_delta`
- **THEN** 具名拒绝（含稳定 token），且响应里没有成本面

### Requirement: The plan body carries the manifest and nothing else, and declares that it is unsigned

升级计划请求体 MUST 只含 `manifest` 一个键，多一个 MUST 拒绝——否则「顺手递一个覆盖价」就把判定输入
交给了调用方。体上限 MUST 从清单上限派生，MUST NOT 沿用命令体口径。本路径 MUST NOT 声称已完成验签：
验签属于 execute 侧，界面 MUST 把这一点与金额一同呈现。

#### Scenario: A body that carries more than the manifest

- **WHEN** 请求体是 `{"manifest": {…}, "override": {"cloud": "0"}}`
- **THEN** 具名拒绝；价格 MUST NOT 由调用方给

#### Scenario: A legal manifest larger than a command body

- **WHEN** 清单约 42KB（远超命令体的 4096，仍在 65536 之内）
- **THEN** 正常算出成本；把命令口径的上限套到本端点上的实现不满足本需求
