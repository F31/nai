## ADDED Requirements

### Requirement: A snapshot database over the declared ceiling is refused before it is read
把快照读回时 SHALL 先按声明上限检查数据库文件的长度；超限的 MUST 在读取任何字节之前以 `resource_exhausted` 具名拒绝，且该拒绝 MUST 与「字节与清单不符」的拒绝**可区分**。

#### Scenario: Over the ceiling and a same-size mismatch are different refusals
- **WHEN** 一份快照的数据库文件被拉长到超过声明上限
- **THEN** 读回以 `resource_exhausted` 拒绝
- **WHEN** 同一尺寸下只改动该文件的字节
- **THEN** 读回以 `conflict` 拒绝，而不是 `resource_exhausted`

### Requirement: Every registered artifact is checked against one declared per-file ceiling
单份登记文件 SHALL 受一个声明上限约束，且**写入**与**读回**两侧 MUST 使用同一上限与同一端点（恰在界上即通过）。

#### Scenario: At the ceiling is accepted and one byte over is refused on both sides
- **WHEN** 写入一份恰为声明上限的文件
- **THEN** 写入被接受
- **WHEN** 写入一份比上限多一字节的文件
- **THEN** 写入以具名拒绝
- **WHEN** 快照中的文件在读回时超过上限
- **THEN** 读回以 `resource_exhausted` 拒绝
