## Purpose
把「仅改配置换 Provider」从一句声明变成可执行判据：凭据呈现与供应商静态头由**协议**派生而非写死在共享传输路径，文本与语音各至少两种线上协议，且替换的证明是**同一个调用函数**在两家协议上跑通、同时线上差异被实测到。

## ADDED Requirements

### Requirement: Protocol-derived credential presentation
供应商的凭据呈现方式与静态头 MUST 由所配置的协议派生，MUST NOT 写死在共享请求路径中。未知协议名 MUST 被拒绝，MUST NOT 落回某个默认供应商。调用方 MUST NOT 需要知道任何供应商特有的头名。

#### Scenario: Two providers present their credentials differently
- **WHEN** 以两种分别使用 `Authorization: Bearer` 与供应商自有凭据头的协议各发一次请求
- **THEN** 各请求携带的凭据头名与协议一致，且**不**同时携带另一协议的凭据头

#### Scenario: A misspelled protocol never selects a default vendor
- **WHEN** 配置中的协议名无法识别
- **THEN** 绑定被拒绝，不发出任何请求

### Requirement: Provider identity is separate from the endpoint
用量记录中的供应商身份 MUST 与 endpoint 地址分离。同一 endpoint 上的两个供应商、或 endpoint 改变后的同一供应商，MUST 能在账本中分辨。

#### Scenario: Identity survives an endpoint change
- **WHEN** 同一供应商身份先后指向两个不同 endpoint
- **THEN** 其用量记录归属同一供应商身份，而不是被记成两家

### Requirement: Two wire protocols per capability, selected by configuration
文本生成与语音合成 MUST 各至少存在两种线上协议适配器，且由配置选择。适配器 MUST 在自身边界内承载该协议的路径、请求体形状、流帧结构与用量字段名；调用方 MUST 通过能力切面调用，MUST NOT 出现供应商名。

#### Scenario: One caller, two text protocols
- **WHEN** 同一个文本生成调用函数先后以两种不同文本协议运行
- **THEN** 两处都返回相同形状的有序块序列与单一终止块，而实际请求的路径与请求体形状不同

#### Scenario: One caller, two speech protocols
- **WHEN** 同一个语音合成调用函数先后以两种不同语音协议运行
- **THEN** 两处都返回相同形状的块序列与终止块，而实际请求的路径与请求体形状不同

### Requirement: Capability mismatch is refused, never degraded
把不具备某能力的协议绑定为该能力时 MUST 显式报错，MUST NOT 降级到其它供应商或本地替身，且 MUST NOT 发出请求。

#### Scenario: Speech-only provider asked for text
- **WHEN** 以语音专用协议构造文本生成客户端
- **THEN** 返回显式错误，且该端点未收到任何请求

### Requirement: Vendor usage maps into the closed canonical units
供应商特有的用量字段名 MUST 在适配器边界映射为既有封闭单位，账本词表 MUST NOT 因此放宽。用量为零的字段 MUST 被省略而非发出零单位。供应商未报告用量时 MUST NOT 为其编造实际值。

#### Scenario: Two vendors, one unit vocabulary
- **WHEN** 两家供应商分别以各自的字段名报告 token 用量
- **THEN** 两者都记入同一组规范单位，且各自归属自己的供应商身份

#### Scenario: A vendor that reports nothing is not credited with a number
- **WHEN** 某供应商在成功响应中不返回任何用量
- **THEN** 该次调用保留估算、不产生实际计费值，且该差异按供应商分别断言而不是被压平成共同期望

### Requirement: Speech is synthesized per sentence and never stitched
一次语音合成调用 MUST 只覆盖一个句子，MUST NOT 跨调用携带序号、缓冲引用或样本偏移。目标采样率 MUST 是供应商能产出的采样率，否则 MUST 拒绝且 MUST NOT 静默替换为另一码率。

#### Scenario: Two sentences stay two sequences
- **WHEN** 先后以两个句子各发起一次合成
- **THEN** 第二次不延续第一次的序号，各数据块的样本起点属于其自身缓冲，且两次各自结算一条用量

#### Scenario: An unproducible sample rate is refused before any request
- **WHEN** 请求的采样率不被该供应商支持
- **THEN** 调用被拒绝，不发出请求、不产出音频块，且预留被释放而非保持 reserved
