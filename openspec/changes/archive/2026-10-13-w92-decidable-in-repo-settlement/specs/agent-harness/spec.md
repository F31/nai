# agent-harness 变更（w92）

## MODIFIED Requirements

### Requirement: A2A inbound is an external request; outbound delegation is bounded

A2A 入站 SHALL 走**独立入口**，入口请求一律按**外部请求**处理：MUST NOT 构成用户指令、MUST NOT 构成授权、
MUST NOT 构成值守；默认只在人工辅助模式下响应，受监督自动模式下响应 MUST 由场景显式启用并预授权。
**R1 首版入站只响应本机与本地信任配置中已登记的受信对端**；对第三方开放 MUST 作为一次**显式的版本决策**，
MUST NOT 因请求自称可信或自称已获授权而放行。
对端身份 MUST 由本地信任配置派生，MUST NOT 取自请求自报；MUST 有作用域、速率限制与体积上限；入站请求 MUST 先过意图判定。
A2A 出站 MUST 声明目标、目的、数据范围与超时，MUST NOT 外发观众原文、知识原文或凭据；返回 MUST 按外部数据标注不可信。
出站 MUST NOT 委派 risk_2 及以上的动作，MUST NOT 委派需要凭据的动作；**R1 首版委派链深度上限 MUST 为常量、
MUST NOT 提供配置面**。

#### Scenario: An inbound peer poses as the user

- **WHEN** 入站请求声称「用户已批准」或自称低风险
- **THEN** 该声明无效力；请求按外部请求处理并进人工队列，MUST NOT 触发外发

#### Scenario: A third party asks to be admitted

- **WHEN** 一个**未登记**在本地信任配置里的对端发起入站请求
- **THEN** 拒绝并**具名**（MUST NOT 以「无响应」代替拒绝）；开放第三方入站 MUST NOT 由请求内容自行开启

#### Scenario: Outbound delegation escalates

- **WHEN** 委派被要求执行 risk_2 及以上动作，或委派链已达深度上限
- **THEN** 拒绝并记录，MUST NOT 继续委派

#### Scenario: Delegation depth is asked to be configured

- **WHEN** R1 首版试图把委派链深度上限做成可配置项（配置面或运行时参数）
- **THEN** 该做法被拒（上限 MUST 为常量）；配置化 MUST 先经一次显式的版本决策

#### Scenario: Credentials would be sent outbound

- **WHEN** 出站委派的数据范围包含凭据或观众原文
- **THEN** 拒绝该委派
