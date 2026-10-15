## MODIFIED Requirements

### Requirement: Forget blocks the read side first and fails by name

遗忘端点 SHALL **先**阻断读取、再清除载荷，并返回**生效中的**撤销时刻（已撤销则返回原有时刻，
MUST NOT 覆盖）。清除结论（副本状态、载荷清除时刻）SHALL 由**系统**给出，MUST NOT 由请求体提供；
请求体 MUST NOT 承载结论字段。记录不存在 SHALL 具名失败，MUST NOT 静默成功。

#### Scenario: Forget blocks the read side first

- **WHEN** 以持 `memory.forget` 的 token 对一条存在的记录请求遗忘
- **THEN** 该行被标为已撤销，随后读取该记录时它进 `refused(revoked)`；
  重复遗忘返回**同一个**撤销时刻

#### Scenario: A missing record is a named failure

- **WHEN** 遗忘请求给的 `record_id` 在本工作区不存在
- **THEN** 响应是具名的失败，MUST NOT 报成功

#### Scenario: The caller cannot declare the purge finished

- **WHEN** 请求体自带 `purge_state` 或 `claims_fully_erased`
- **THEN** 该请求按**未知字段**被拒（`400`）；MUST NOT 静默忽略一个自称已擦除的字段，
  MUST NOT 照抄成响应里的结论

## ADDED Requirements

### Requirement: The copy-site ledger is a closed world derived from the code

控制面 SHALL 持有一份**记忆载荷副本清单**，每个在册点 MUST 有逐字存在的清除体。
清单 MUST 与**代码派生**的落点集合一致：新增一个承载记忆载荷的落点而未登记，判据 MUST 失败。
清单为空 MUST 被**清点**并报出 `0`，MUST NOT 当成「无副本可清」而跳过。

#### Scenario: An empty ledger is counted, not skipped

- **WHEN** 清单当前没有任何在册副本点
- **THEN** 清除仍执行清点并报 `copy_sites_cleared = 0`，副本状态得出「已清」；
  把空清单短路成「跳过」的实现不满足本需求

#### Scenario: A new payload landing that is not registered fails the guard

- **WHEN** 存储层新增一处写入记忆载荷的落点而清单未登记
- **THEN** 闭世界判据失败并点名该落点；MUST NOT 因为清单是手写而放行

### Requirement: Erasure is claimed at the scope that was actually achieved

清除响应 SHALL 声明擦除**口径**：`scope` 为 `logical`、`physical_bytes_erased` 为 `false`。
本轮 MUST NOT 宣称介质级完全擦除——置空一列不是介质级擦除（页与 WAL 可能仍留有字节）。

#### Scenario: The response never claims media-level erasure

- **WHEN** 一次清除成功完成
- **THEN** 响应里的 `erased.physical_bytes_erased` 为 `false` 且 `scope` 为 `logical`；
  把它写成 `true` 的实现不满足本需求