## ADDED Requirements

### Requirement: The ledger entry encoding bound has two sides at its gate
账本条目编码 SHALL 受一个声明上限约束：恰在界上 MUST 编得出来，越一字节 MUST 以 `invalid_argument` 具名拒绝。该上限 MUST 被如实标明为**防御性**——若今日被认可的路径装不下它，则 MUST 有一条**会过期**的断言钉住「当前最大合法条目离上限多远」。

#### Scenario: Both sides of the write gate and a reachability statement that can expire
- **WHEN** 编码一份恰为声明上限的条目
- **THEN** 编码成功
- **WHEN** 编码一份比上限多一字节的条目
- **THEN** 以 `invalid_argument` 具名拒绝
- **WHEN** 一份把校验允许的每一格都顶满的条目被写入并读回
- **THEN** 它远低于写入上限，且这一「远低于」被断言（该校验一旦放松，断言即失效）

### Requirement: The stored ledger text bound is inclusive at its ceiling
读回一份已存账本文本时 SHALL 先按其长度检查声明上限；恰在界上 MUST 读得回来，越界 MUST 以 `invalid_argument` 具名拒绝。

#### Scenario: At the ceiling reads back and one byte over is refused
- **WHEN** 改写已存账本文本使其长度恰为声明上限
- **THEN** 读回成功
- **WHEN** 再改写使其比上限多一字节
- **THEN** 读回以 `invalid_argument` 具名拒绝

### Requirement: The stored ledger hex window is a gate of its own
读回已存账本时，其 `protobuf_hex` 的**字符数** SHALL 受一个声明上限约束，且该上限 MUST 与上述文本长度上限**是两道闸门**：奇长度、超长、非十六进制 MUST 各自以具名拒绝；恰在界上的偶数十六进制 MUST 不因长度被拒（其拒绝来自下一道「解不开」闸门）。

#### Scenario: Each of the three conditions is attributable, and the ceiling falls through to the decoder
- **WHEN** 十六进制长度恰为声明上限且字符合法
- **THEN** 该长度闸门不开火，拒绝来自 `invalid stored ledger protobuf`
- **WHEN** 十六进制长度比上限多两个字符（仍为偶数）
- **THEN** 以 `invalid stored ledger bytes` 具名拒绝
- **WHEN** 十六进制长度为奇数
- **THEN** 以 `invalid stored ledger bytes` 具名拒绝
- **WHEN** 十六进制长度在界内但含非十六进制字符
- **THEN** 以 `invalid stored ledger bytes` 具名拒绝
