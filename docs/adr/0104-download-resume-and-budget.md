# ADR 0104：升级下载阶段的续传、重问预算与哈希闸门

- 状态：Accepted｜日期：2026-10-16｜归属：W126（R09「升级预检与下载失败」的下载阶段）
- 关联：ADR 0026（升级器边界）、ADR 0004（上界外部事实）、ADR 0099（`Operation` 有界预算）、
  `upgrade-transaction` spec、R09/W15、`admission-registry` §Honest backfill mapping with traceable evidence

## 背景
R09 的缺口自 W15 起记为「真实下载器（断流/重试/校验失败）未实现，只覆盖内容到达后的校验与预检」。
本次勘定把这句话拆开看，发现它同时是**架构性**的与**判定性**的：

- `nai-upgrade-core` 的每个入口都只**接收**字节——`verify_package(… files: &[PackageFile])`、
  `install_offline(… artifacts: &[OfflineArtifact])`、`UpgradeTransaction::prepare(request.files: Vec<PackageFile>)`。
  全 crate 没有一处决定「向哪一段要、短读要不要重问、什么时候放弃」。
- 因此「网络断开与下载中断时原版本继续运行，可续传并校验」（基线 §14.3/§1322）在实现与判据两侧都不存在：
  断流只能表现为「字节没到」，而「字节没到」与「下载失败」在类型上是同一件事——**没有可判的区分**。
- 架构上不能把 HTTP 客户端塞进本 crate：它是 `shared` 层，`lib.rs` 与 `offline.rs` 都把
  「无宿主 / 无平台 SDK / 无网络依赖」写成**刻意**的性质（R11 亦明确否决 `allow_fetch` 开关）。

## 决策
**下载的决策留在 core，传输与本地持久化都做成接缝。**

1. 新增 `download` 模块。世界事实两条：`PackageFetcher`（一个窗口的字节到不到）与
   `ResumeLedger`（本地已持久化多少）。core 只做决定，不持有传输。
2. **短读是值，`Err` 是终局**。`Fetched.bytes` 允许短于窗口；短读可重问并**被清点**，
   传输返回的错误**不重问**。判据：重问耗尽 ⇒ `resource_exhausted`（具名），
   终局错误一次也不重问。
3. **续传偏移每次从台账读回**，append 之后**回读确认**；哈希闸门的输入是 `ledger.assembled(path)`。
   于是「工件与声明相符」是关于**本地介质**的陈述，而不是关于本次调用持有过哪些缓冲的陈述。
   超出声明的续传点 ⇒ `invalid_argument`，**不夹取**。
4. **完整但哈希不符 ⇒ 拒绝 + 先丢弃**。理由不是洁癖，而是续传点本身：坏前缀若留下，
   下一次运行会在坏文件的末尾续传，而那个位置永久不会被重取（重启也带不走）。
   判定词沿用校验链的 `package content hash mismatch`——**一条校验链，不是两条**。
5. **声明的上界在动一个字节之前应用**（`bound_declared`）。判定词与 `verify_package` 相同，
   但**不是同一条规则**：那条约束「实际到达」，这条约束「即将索要」——两个不同输入，故不共享函数。
6. 上界取 `FetchPolicy::max_attempts_per_window`，默认 **3**；停止信号为入口一条 `checkpoint`
   加每个窗口循环内的 `op.check()`。

## 后果
- 正向：断流 / 续传 / 重问预算 / 下载期校验从「零判据」变为被判别性断言钉住的行为，
  且**每条守卫都有独立注入使其变红**（10 条注入，逐条可归因）。
- 正向：两条判据走在**真实 loopback TCP/HTTP/1.1** 上，wire 上真正发出的 `Range: bytes=N-`
  就是续传偏移的证据；而**零新增依赖**（peer 与 client 都是测试内最小实现）
  ⇒ 「本 crate 无 HTTP 客户端」这句话**连 dev-dependency 层面仍逐字成立**。
- 零契约字段改动、零迁移、零 schema 改动、零生产行为改动（新增模块此前不存在）。
- **如实披露（非验收达标）**：
  - 本轮补的是下载**编排**与故障语义的判定面与载体。真实分发端点、真实安装器、
    真实磁盘与 GPU 度量仍是外部注入点 ⇒ `R09` **维持 `implemented`**，不升 `tested`
    （第五次使用「守卫通过 ≠ 本项达标」）。
  - 测试内的 peer 是自写的最小 HTTP/1.1 实现：它证明的是本仓库的下载编排对真实 socket 的行为，
    **不是**与第三方服务器实现的互通。
  - `assembled()` 把整个工件取回内存，与既有 `verify_package` 同形；大工件的流式校验不在本轮范围。
  - 本轮**删掉**一条重复守卫（`bound_declared` 之后那条 `checkpoint` 与入口那条同规则、无独立可观测差异），
    并按「新守卫 MUST 被注入验证」为两条停止守卫各补了一条判别性判据。
