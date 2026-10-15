## ADDED Requirements

### Requirement: Remote delete contract enforces input bounds and never claims third-party erasure
远端删除 SHALL 以 `RemoteGateway::delete` 传输契约执行：删除目标 id 列表 SHALL 非空、≤16、且每个 id 非空，否则 SHALL 以 `Unauthorized` 拒绝且不触网。远端回执 SHALL 原样透传，系统 SHALL NEVER 将 `RemoteReceiptStatus` 的任何变体解释为「第三方副本已擦除」的证明——`RemoteReceiptStatus` 的有界集合（accepted/pending/deleted/retained）SHALL 不含任何「已擦除」语义。删除前的本地 tombstone 由存储层 `remote_delete_targets` 负责（§24.4），传输契约假定其已存在。

#### Scenario: Malformed delete targets are rejected without network
- **WHEN** 删除目标为空列表、超过 16 个、或含空 id
- **THEN** `RemoteGateway::delete` 返回 `Unauthorized` 且不向远端 `/delete` 端点发起请求

#### Scenario: Remote receipt is passed through and never claims erasure
- **WHEN** 远端返回 `RemoteReceipt{status: deleted, ...}`
- **THEN** 网关原样返回该回执（`request_id` / `policy_ref` / `status` 透传），且 `RemoteReceiptStatus` 的穷尽匹配仅含 accepted/pending/deleted/retained 四态，无任何「第三方已擦除」语义
