## ADDED Requirements

### Requirement: Approval actions act only on candidates that are still valid

桌面审批动作 MUST 在发出请求前复核候选仍然有效：候选哈希仍在**最新列出**的队列中、其 `expires_at_utc_ms`
未过期、且当前会话 `generation_epoch` 与列出该候选时一致。任一条件不满足时 MUST NOT 发出请求，
MUST 报出具名理由，并 MUST 要求重新列出而非沿用旧快照。界面 MUST NOT 依据列出时的快照直接批准。

#### Scenario: Expired candidate

- **WHEN** 候选的 `expires_at_utc_ms` 已过（或载荷未给出有效期）
- **THEN** 不发出请求，报出「已过期／有效期未知」，该候选的批准动作关闭

#### Scenario: Session generation changed since listing

- **WHEN** 动作前重新读取的会话 `generation_epoch` 与列出候选时不同
- **THEN** 不发出请求，报出「会话已换代」，并需重新列出

#### Scenario: Candidate no longer listed

- **WHEN** 候选哈希已不在最新列出的队列中（已被处理或不再有效）
- **THEN** 不发出请求，报出「候选已不在队列」

### Requirement: High-risk approval needs a second confirmation and declares what it cannot show

批准（高危方向）MUST 需要第二次显式确认：单次点击 MUST NOT 发出请求。确认面 MUST 声明本载荷
**不提供候选的最终文本**、身份为候选哈希，并 MUST 指向该缺口对应的验收项；界面 MUST NOT 使操作员
误以为已经看到将要播报的文本。拒绝（安全方向）不要求二次确认。

#### Scenario: A single click does not approve

- **WHEN** 操作员点击一次批准
- **THEN** 不发出请求，界面进入待确认态；再次点击同一候选才提交

#### Scenario: The unavailable final text is declared

- **WHEN** 确认面呈现一个候选
- **THEN** 明确声明其最终文本不在本载荷内、身份是候选哈希，并给出该缺口对应的验收项

### Requirement: The desktop UI layer declares each assertion's implementation status

§30.1 指定的 UI 层 SHALL 位于 `tests/ui`，且 MUST 逐条声明其四条断言
（审批失效、模式切换条件、静音 ACK、升级与回滚提示）的实现状态。对界面路径尚未实现的断言，
MUST 以**未实现**记录并点名缺失前置，MUST NOT 断言其通过；声明的断言集合 MUST 恰为这四条，
既不得漏登记，也不得臆造未登记项。

#### Scenario: An unimplemented assertion is declared, not faked

- **WHEN** 某条断言所需的界面路径不存在（模式切换需 `POST /sessions/{id}/mode`，升级提示需升级状态来源）
- **THEN** 该断言以未实现记录并点名缺失前置，测试对其断言「未实现」而非通过

#### Scenario: The declared set is exactly the four

- **WHEN** 枚举 UI 层声明的断言
- **THEN** 恰为 §30.1 列出的四条，无遗漏、无未登记项
