## ADDED Requirements

### Requirement: Randomized untrusted-input testing of the IPC surface

IPC 帧层、握手与信封解码 SHALL 由**确定性、有界、可复现**的随机输入测试覆盖，语料种子固定。
对**任意**字节序列，解码路径 MUST 只产生 `Ok` 或**具名 `ErrorCode`** 的错误——MUST NOT panic，MUST NOT 依据输入无界分配，
MUST NOT 存在第三种结局。测试 SHALL 自带**非空洞性守卫**：语料必须实际命中约定的危险形状与具名错误码，计数不足即测试失败。

#### Scenario: Arbitrary bytes at the frame boundary

- **WHEN** 把任意长度的任意字节序列交给帧读取（含 `0xFFFFFFFF` 长度前缀、截断 body、截断 header、合法帧后跟任意字节）
- **THEN** 结果为 `Ok(Some)`、`Ok(None)` 或具名错误；长度前缀超过上限时返回 `RESOURCE_EXHAUSTED` 且不按声明的长度分配；过程中不 panic

#### Scenario: Structured envelopes with arbitrary payloads

- **WHEN** 一个通过握手的连接收到「协议版本合法、`message_type` 与 payload 任意」的帧序列
- **THEN** 宿主以具名错误结束该连接（版本不符 ⇒ `VERSION_INCOMPATIBLE`；未知消息类型或畸形 payload ⇒ `INVALID_ARGUMENT`；未授予的调用 ⇒ `PERMISSION_DENIED`/`CAPABILITY_ABSENT`），不 panic、不继续按原语义分发

#### Scenario: The corpus is not vacuous

- **WHEN** 随机输入测试运行完毕
- **THEN** 断言语料确实命中了约定的形状与错误码（至少各一次：超长长度、截断、合法往返、版本不符、未知消息类型、进入能力调用解码、路径穿越被拒与合法路径通过）；任一计数为零即测试失败

### Requirement: Cancellation and deadline precede content interpretation

取消或期限到期时，帧读取与信封分发 MUST 在解释内容**之前**返回 `CANCELLED` 或 `DEADLINE_EXCEEDED`，
SHALL NOT 先成功解析内容再检查。该顺序对**任意**输入成立，不因输入形状而异。

#### Scenario: Revocation wins over any content

- **WHEN** 共享取消标志已置位（或期限已过）且读端仍有任意字节
- **THEN** 返回 `CANCELLED`（或 `DEADLINE_EXCEEDED`），且不返回任何解析结果

### Requirement: Path containment holds under randomized inputs

路径包含校验 SHALL 在**随机组合**的路径字符串下仍保证不出根目录：任何含 `..` 段的输入 MUST 被拒，
返回的规范化路径 MUST 位于规范化的根目录之下。测试语料 SHALL 实际包含穿越形状，并以计数守卫其非空洞性。

#### Scenario: Randomized traversal corpus

- **WHEN** 由 `..`、绝对路径、空段、分隔符变体与长段随机组合出的路径被校验
- **THEN** 含 `..` 段者一律被拒（`PERMISSION_DENIED`），通过者的规范化路径必在根目录之下，且语料中「被拒」与「通过」两类均至少出现一次
