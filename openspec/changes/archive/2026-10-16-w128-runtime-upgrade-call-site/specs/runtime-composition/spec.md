## ADDED Requirements

### Requirement: The composition root drives an upgrade through staging and a gated activation

本需求规定的是**装配根这一侧的接缝形状**（升级策略本身见 `upgrade-transaction`，此处不重述）。

装配根 SHALL 提供两处入口驱动一次升级事务：**暂存**与**激活边界**。暂存 SHALL NOT 询问升级激活门
（暂存在任何时候都是安全的；只有运行中版本的切换需要静默）。激活边界 SHALL **先**问门，且该判定 MUST 由
装配根**自己持有的真实会话**回答，MUST NOT 另立第二处判定。门答「推迟」时 MUST 返回一个**非错误**的推迟
结局并带上活跃会话数；被推迟的已暂存包 MUST 仍然有效，且 MUST 在下一个静默边界被**复用**而不是被二次暂存。
门**无法观测**会话时 MUST 返回错误，MUST NOT 把「未知」表述为推迟或放行。事务使用的 `Operation` 预算
SHALL 由调用方给出。

#### Scenario: A live show stages but does not swap

- **WHEN** 装配根的真实会话在播（Running 或 Draining）时一个包被暂存，随后到达激活边界
- **THEN** 暂存成功；激活边界返回**推迟**（活跃数为 1）而不是错误，且宿主上没有发生备份、迁移或切换
  （效果计数**恰为**一次暂存，切换记录为空），日志仍读作 `prepared`

#### Scenario: The deferred package is what the next silent boundary swaps

- **WHEN** 在播期间被推迟的同一个已暂存包，在其会话停止后被再次送进激活边界
- **THEN** 事务以 `committed` 结束，切换恰发生一次，且暂存计数仍为 1（包被复用，未二次暂存）

#### Scenario: An unobservable session fails the swap but not the staging

- **WHEN** 会话锁在激活边界被调用时已被占用（竞争已就位），而暂存请求在同一时刻到达
- **THEN** 暂存仍然成功（它不读会话状态），激活边界返回 conflict 错误；宿主未发生切换，包仍为 `prepared`

### Requirement: A completed activation is reported with the core's own outcome, and a deferral is never one

激活边界 SHALL 把「事务运行后的终局」与「被推迟」分开报告：运行后的终局 MUST **原样**携带核心给出的
`Outcome`（`Committed`，或带**具名成因**与宿主**导出计数**的 `RolledBack`），MUST NOT 在运行时侧重写、
补零或改写其中任一字段。入口的返回 MUST 可区分「已运行」与「被推迟」，MUST NOT 把一次已经发生并已收敛的
回滚表述为「等待」——那会同时丢掉成因与导出计数，并承诺一次永远不会修好它的重试。

#### Scenario: A rolled-back run is reported as a run, not as a wait

- **WHEN** 没有在播会话，而激活中的迁移步骤失败，且宿主持有升级后写入的导出计数
- **THEN** 返回「已运行」且 `Outcome` 为具名回滚：成因码为迁移失败的码，导出计数为宿主给出的那一份；
  宿主恰发生一次回滚、零次切换，当前版本清单未被替换
