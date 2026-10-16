# Proposal: W126 — 升级下载阶段的续传、重试预算与哈希闸门（R09 进展）

## Why
R09 的 registry notes 自 W15 起写着「真实下载器（断流/重试/校验失败）未实现，只覆盖内容到达后的校验与预检」。
实测确认这句是准的，而且原因比「没写」更细：`nai-upgrade-core` 的每个入口都只**接收**字节
（`verify_package` 收 `&[PackageFile]`、`install_offline` 收 `&[OfflineArtifact]`、`prepare` 收
`Vec<PackageFile>`）——**没有任何一处决定「向哪一段要、短读要不要重问、什么时候按名放弃」**。
于是基线 §14.3/§1322 的「下载中断时可续传并校验」在实现与判据两侧都不存在：断流只能表现为「字节没到」，
而「字节没到」与「下载失败了」在类型上是同一件事。

## What Changes
1. 新增 `download` 模块，把下载的**决策**留在 core：窗口从哪个偏移开始、短读是否重问、何时按名放弃。
   两个世界事实走接缝：`PackageFetcher`（传输）与 `ResumeLedger`（本地已持久化多少）。
2. 续传偏移**每次从台账读回**，append 之后**回读确认**，哈希闸门的输入也**读自台账**——
   于是「工件与声明相符」是关于本地介质的陈述，而不是关于本次调用持有过哪些缓冲的陈述。
3. 短读是**值**（可重问、被清点）；`Err` 是**终局**（不重问）。重问受 `FetchPolicy` 上界约束，耗尽 ⇒ 具名拒绝。
4. 完整但哈希不符的工件 ⇒ 拒绝，且**先丢弃**其持久化字节：留着会让下一次从坏文件末尾「续传」，
   而那个位置永远不会被重取（重启也带不走的毒）。
5. 新增 `tests/download.rs` **20** 条判据，其中 **2** 条走**真实 loopback TCP/HTTP/1.1**
   （测试内自写最小 peer 与 client，**零新增依赖**）：wire 上真正发出的 `Range: bytes=N-` 就是续传的证据。

## Modified Capabilities
- `upgrade-transaction`：下载阶段 MUST 由**存储读出的**持久化状态续传；重问 MUST 有界且耗尽 MUST 具名拒绝；
  完整但哈希不符的工件 MUST 被拒绝且 MUST NOT 留下可被续传的坏前缀。

## Impact
- registry：`R09` **维持 `implemented`**——本轮补的是下载**编排**与故障语义的判定面与载体；
  真实分发端点、真实安装器与真实磁盘/GPU 度量仍是外部注入点，按 `admission-registry` 不升 `tested`。
- 零新增依赖、零契约字段改动、零迁移、零 schema 改动、零生产行为改动（新增模块此前不存在）。
