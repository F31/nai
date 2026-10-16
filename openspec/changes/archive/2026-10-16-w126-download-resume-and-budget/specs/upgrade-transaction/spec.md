## ADDED Requirements

### Requirement: An interrupted download resumes from durable state, read from storage

下载中断后 MUST 从**已持久化到本地介质**的字节数续传，且该偏移 MUST 由**读取存储**得出，
MUST NOT 取自调用方传入的进度或编排器自己的累计。每次追加后 MUST **回读**确认持久化确实前进。
写入的窗口 MUST NOT 越过其声明终点；一个**超过声明大小**的续传点 MUST 被具名拒绝，
MUST NOT 被夹取（clamp）成「已完成」或「从头开始」。

#### Scenario: A short read is resumed, not restarted
- **WHEN** 某个工件的第一次响应只送到一部分字节，随后连接中断
- **THEN** 下一次请求的偏移等于该工件**已持久化**的字节数
- **AND** 已持久化的那部分 MUST NOT 被重新请求

#### Scenario: A ledger that does not persist is a failure, not progress
- **WHEN** 追加写入报告成功但回读的持久化字节数没有前进
- **THEN** 下载以具名内部错误失败，MUST NOT 把该窗口记作已到达

#### Scenario: A resume point past the declared end is refused
- **WHEN** 台账报告的某工件持久化字节数超过该工件的声明大小
- **THEN** 下载以 `invalid_argument` 拒绝，且不发出任何窗口请求

#### Scenario: Content already durable is verified without moving a byte
- **WHEN** 某工件的全部声明字节已持久化
- **THEN** 不请求任何窗口
- **AND** 该工件仍 MUST 通过其声明哈希（哈希的输入是持久化内容，不是本次运行的缓冲）

### Requirement: Re-asking is bounded and exhaustion is a named refusal

一次下载 MUST NOT 无限重问。**短读**（到达字节少于所请求窗口）是**可重问**的取值并被清点；
传输返回的错误是**终局**，MUST NOT 被重问。每个窗口的重问次数受一个声明上界约束，
**恰在上界内**完成 MUST 成功，**越过上界** MUST 以 `resource_exhausted` 具名拒绝并停止。

#### Scenario: The last allowed attempt may still succeed
- **WHEN** 重问上界为 2，且第二次尝试收到完整窗口
- **THEN** 下载成功，且实际请求次数恰为 2

#### Scenario: A source that never completes stops by name
- **WHEN** 每次尝试都只送到一部分字节，且重问次数已达上界
- **THEN** 下载以 `resource_exhausted` 拒绝，且请求次数恰为上界的值

#### Scenario: A transport error is not re-asked
- **WHEN** 传输返回一个错误而不是短读
- **THEN** 该错误被原样传播，MUST NOT 消耗重问预算

#### Scenario: More than the window is refused
- **WHEN** 一次响应到达的字节多于所请求的窗口
- **THEN** 下载以 `invalid_argument` 拒绝，MUST NOT 把多余字节接在工件上

### Requirement: A complete artifact that fails its declared hash is refused and discarded

一个字节数已达声明大小、但哈希与签名声明不符的工件 MUST 被拒绝，
且该工件在本地介质上的持久化字节 MUST 先被**丢弃**并回读确认已清空。
丢弃的理由是续传点本身：若坏前缀留下，下一次运行会在坏文件的末尾续传，而那个位置永远不会被重取。

#### Scenario: Corrupt bytes are refused and do not survive as a resume point
- **WHEN** 某工件的全部声明字节已到达，但其哈希与声明不符
- **THEN** 下载以 `invalid_argument` 拒绝（与校验链同一判定词）
- **AND** 该工件的持久化字节数回读为 0

#### Scenario: The next run refetches from zero instead of inside the discarded bytes
- **WHEN** 在被拒绝之后用同一份台账重跑该下载
- **THEN** 第一个窗口的偏移是 0，而不是坏前缀的长度
