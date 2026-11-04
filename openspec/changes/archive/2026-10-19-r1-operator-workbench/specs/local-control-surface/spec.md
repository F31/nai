## MODIFIED Requirements

### Requirement: High-risk approval needs a second confirmation and declares what it cannot show
批准 MUST 需要第二次显式确认，单次点击 MUST NOT 发出请求。审批列表与确认面 SHALL 展示载荷中的最终文本；文本缺失时 MUST 展示具名缺口，不得用哈希冒充文本。身份始终为候选哈希。拒绝不要求二次确认。

#### Scenario: A single click does not approve
- **WHEN** 操作员点击一次批准
- **THEN** 不发出请求；再次点击同一候选才提交

#### Scenario: Final text is available
- **WHEN** 候选包含非空 display_text
- **THEN** 审批列表与确认面展示该文本，同时保留候选哈希身份

#### Scenario: The unavailable final text is declared
- **WHEN** 候选没有可展示文本
- **THEN** 展示 FINAL_TEXT_GAP，不以哈希冒充文本

## ADDED Requirements

### Requirement: Operators can query upgrade costs and durable job status from the desktop
桌面 SHALL 提供包清单成本查询和显式作业 id 状态查询，调用现有 ControlClient.planUpgrade/fetchJob。成本 SHALL 复用既有逐项与未知呈现并显示未验签声明；作业 SHALL 复用既有状态说明。查询前清理旧结果，失败显式呈现；切换连接或断开清理全部结果，迟到响应 MUST NOT 恢复旧结果。缺安装器时 MUST 明示执行与回滚不可用。

#### Scenario: Operator queries a manifest
- **WHEN** 操作员提交合法 JSON 清单
- **THEN** 页面展示服务端返回的四类成本和总额口径，不创建作业 id

#### Scenario: Operator queries a job
- **WHEN** 操作员提交作业 id
- **THEN** 页面展示真实响应，或具名不可用/不存在错误，不伪造事务进展

#### Scenario: Failed or obsolete query
- **WHEN** 查询失败，或响应前连接代次已变
- **THEN** 不展示该次查询的成功结果，旧成本或状态也不冒充当前结果
