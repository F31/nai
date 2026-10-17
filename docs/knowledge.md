# nai 知识链路（D09 完成）

## 算法与传输入口

- `nai-knowledge-core::tokenize`：中文 unigram/bigram 与混合词 token，流式处理、最多 4096 项。
- `split_document`：线性分块，保留行列 span、product_scope、acl_ref，最多 4096 块。
- `vector::VectorSpace`：维度/归一化校验、cosine 排名和有界 top-k；`rank::fuse` facts 优先并去重。
- `RemoteKnowledgeClient`：严格 DTO 校验的 capabilities/health/search/answer/sync/delete 传输。

## 发布闭环

`Store::import_document` 接受已解析文本与分块，写入准备区，**普通检索不可见**。
空文本拒绝，内容/分块内容合计与 token 总量分别最多 16 MiB，单块 token 最多 4096。
workspace_id 与 product_scope 独立。原始 content_hash 为 SHA-256；document revision 还绑定 parser、
source、ACL、有效期与分块内容，改元数据也会生成新 revision。
重导入更新准备区并清理其旧向量，不修改已发布副本。

```rust
store.import_document(&op, actor, &parsed_document)?;
store.stage_snapshot(
    &op, actor, workspace_id, snapshot_id, kb_scope,
    &reviewed_document_ids, None, previous_active_id,
)?;
let sealed = store.index_snapshot(&op, actor, workspace_id, snapshot_id)?;
store.publish_snapshot(&op, actor, &sealed)?;
```

上例为词法-only。需要向量时，经允许的 D08 API 生成并 `index_vectors` 写入准备区，stage 时传 index_ref。
向量 provider/model/dimension/normalization/chunker/content_hash identity 不可用同 ID 改写；
维度最多 8192、非有限值/全零拒绝，缺失 chunk 外键使批次回滚。
seal 要求向量覆盖所有选定块且与捕获 revision 匹配；失败不留下部分 FTS/向量副本。

stage 捕获选定文档、facts、权限版本与 previous_active_id；每快照最多 256 文档、4096 块，
文本、token、向量和 facts 各最多 16 MiB。stage/index 成功仍不可见，重启也不自动发布。
服务器 manifest 绑定文档/来源/ACL/有效期、块/token、完整向量 identity/bytes 和事实版本。
publish 同事务核对 manifest、workspace 当前事实 fingerprint、权限、活动指针和源 revision，
切换活动快照、失效旧依赖并写审计；失败保持原版本。重复发布旧已发布 ID 不回切活动指针。

## 会话、检索与候选

- `retrieve_local`：只读该 workspace/kb_scope 当前已发布快照；没有活动版本为 unavailable。
- `bind_session_knowledge`：一次绑定当前活动已发布版本，返回已有 SessionSnapshot.knowledge_snapshot_ref。
- `retrieve_session_knowledge`：只读冻结 ID；新的发布不重写绑定，失效证据也不偷偷换成新版本。
- `record_candidate`：校验引用来自冻结快照，记录 doc_revision；不能把最新 facts 塞入旧绑定。
- `approve` / `approval_valid`：复检文档期限、tombstone、session/epoch/hash、事实与审批权限。

SQL/向量候选均复检产品、文档/分块 ACL 和时间；facts 必须仍匹配当前 revision。
Evidence 带原文、来源 span、revision、有效期、产品和 permission_revision；facts 带 RevisionRef。
正常完成有证据为 complete，无合法候选为 empty；缺失所请求向量但已有合法词法/facts 时为 partial，保留结果。
本地取消/期限通过 StorageError 返回，不能误转换为 empty。

替换/移除写 kb-scoped revision tombstone，失效关联待播/审批并保守清缓存；同文档另一知识库仍可合法引用。
显式 `revoke_knowledge` 写 workspace-wide tombstone，再清索引与关联待播审批。
这不代表真实媒体进程已静音或第三方副本已擦除。

## 远端适配与本地政策门控

正式协议位于 `schemas/openapi/remote-knowledge.yaml`，与桌面 control API 独立；生成 Rust DTO、TS types、
内嵌 JSON Schema 与端点常量，生成代码不手改。`RemoteGateway` 由 trusted composition root 注入
`RemotePolicy`，每次调用复检授权过期、允许操作、KB 白名单，以及 query/document 上传、第三方留存、删除要求；
capabilities/留存声明只作能力约束，不当权限。answer_only 显式返回 human-reference 标志，不自动外发。
search/answer 有界只读重试；sync/delete 单次尝试，避免未知提交重复，返回回执而非第三方已擦除的承诺。
（W113 已把此不变量钉成可测判据：`RemoteGateway::delete` 的输入边界闭世界——空集/>16/含空 id ⇒ `Unauthorized` 不触网——与回执透传枚举闭世界守卫——`RemoteReceiptStatus` 穷尽四态、新增 `erased`/`purged` 变体会编译失败；见 [ADR 0091](adr/0091-remote-retention-delete-contract.md)、[W113 报告](reports/2026-10-15-w113-remote-retention-delete-contract.md)。`max_retention_seconds` 声明但本地无强制机制，如实记为遗留。）
（W114 已把远端网关的构造期与输入边界闭世界钉成可测判据：构造期——`RemoteGateway::new` 拒绝 policy 六类非法变体、`RemoteKnowledgeClient::new` 拒绝 endpoint/timeout/max_retries 五类非法变体（均 `Invalid` 不触网）；输入边界——`RemoteGateway::sync` 文档集空/>16/单文档 content>65536 ⇒ `Unauthorized` 且显式证明仅合法调用触网，`RemoteGateway::capabilities` 仅当首个 `allowed_operations` 为 Search/Answer 才查询远端（否则 `Unauthorized` 不触网）；见 [ADR 0092](adr/0092-remote-gateway-construction-input-bounds.md)、[W114 报告](reports/2026-10-15-w114-remote-gateway-construction-input-bounds.md)。`RemoteGateway::new` 不校验 `allowed_operations`、`capabilities` 守卫依赖「首操作」约定，如实记为当前语义边界。）
（W115 已把检索证据闭世界钉成可测判据：`rerank_evidence` 空 base 直接短路返回 `RetrievalEmpty`、不进入 `spawn_blocking`（用 panic reranker 探针证明未被调用）；reranker 返回重复 `evidence_id` 时只保留首个（`seen: BTreeSet` 去重）；reranker 闭包 panic ⇒ `JoinError` ⇒ `RemoteError::Invalid`（不静默降级为 partial/empty）；`to_evidence` 丢弃 `evidence_id` 为空的远端项（被 `search` 的 `filter_map` 掉，经公共 `search` API + Mock 覆盖）；见 [ADR 0093](adr/0093-retrieval-evidence-closed-world.md)、[W115 报告](reports/2026-10-15-w115-retrieval-evidence-closed-world.md)。unknown-id drop（W114 既有）与重复 id 去重是独立闭世界维度，分别由 `known`/`seen` 两个 `BTreeSet` 负责。）
（W116 已把 `RemoteError::status()` 公开契约映射钉成可测判据：5 个错误变体 → `RetrievalStatus` 的映射此前整段零判据，含两条非显然坍缩——`Invalid → RetrievalUnavailable`（无独立 `RetrievalInvalid`）、`Timeout|Cancelled → RetrievalTimeout`（两语义并入同态）；新增 4 个同步 `#[test]` 覆盖全映射（Unauthorized→Unauthorized、Unavailable→Unavailable、Invalid→Unavailable、Timeout+Cancelled→Timeout），与既有 async 测试隔离；见 [ADR 0094](adr/0094-remote-error-status-closed-world.md)、[W116 报告](reports/2026-10-15-w116-remote-error-status-closed-world.md)。`status()` 与 `RetrievalStatus` 消费侧（gateway `parse_reply`）解耦，W116 只钉声明侧映射正确性，不引入新状态分支。）
（W117 已把检索结果 `top_k` 截断钉成可测判据：`parse_reply`（remote.rs:395-411）的 `filter_map(to_evidence).take(top_k)` 此前整段零判据；两条非显然语义——**filter 先于 take**（`top_k` 统计丢弃空 `evidence_id` 后的有效项数，非原始回包项数，用判别性构造 `[e0,"",e1,e2]`+top_k=2 ⇒ `[e0,e1]` 与 take-then-filter 的 `len=1` 二分）、**take 保前 N 不补齐**（5 项+top_k=2 ⇒ `[e0,e1]`、top_k=0 ⇒ 空、top_k=10 > 5 项 ⇒ 全保留）；经公共 `gateway.search` + Mock 覆盖私有 `parse_reply`；见 [ADR 0095](adr/0095-retrieval-top-k-closed-world.md)、[W117 报告](reports/2026-10-15-w117-retrieval-top-k-closed-world.md)。`gateway.answer` 亦调 `parse_reply`（624）与本轮共用同一函数；只钉既有截断语义，`top_k` 是否设上界属独立缝。）
（W118 已把 `parse_reply`（remote.rs:395-411）的 `RemoteStatus → RetrievalStatus` 6 臂映射钉成可测判据：`remote.rs:396-403` 把 body `status`（complete/partial/empty/timeout/unavailable/unauthorized）映到同名 `RetrievalStatus`，此前经 `parse_reply` 只覆盖 Complete/Empty 两臂；**关键**——既有 `RetrievalTimeout`/`RetrievalUnavailable`/`RetrievalUnauthorized` 断言均来自 `search` 的 HTTP 内联错误分支（remote.rs:317-327），**不是** body status 映射（两路径独立），换错映射不可见。新增 4 个测试经 **HTTP 200 body status** 隔离映射（经公共 `gateway.search`+Mock），补齐 partial/timeout/unavailable/unauthorized 四臂，合计 6/6；见 [ADR 0096](adr/0096-remote-status-mapping-closed-world.md)、[W118 报告](reports/2026-10-15-w118-remote-status-mapping-closed-world.md)。`gateway.answer` 亦调 `parse_reply`（624）共用；本轮不改 `remote.rs`，不合并内联分支与 body 映射。）
（W119 已把**出站请求构造**（remote.rs:100-112 `request()`）钉成可测判据：既有测试经 `Mock::client()` 恒传 `api_key: None` 且 endpoint 无尾斜杠 ⇒ 凭证头与 endpoint 规范化整段零判据。新增 4 测试配**记录型 Mock**（`ObservedRequest` 记录 `url` + `Authorization`，既有 Mock 是「响应型」、看不到出站请求内容）：`api_key=Some` ⇒ 必带 `Authorization: Bearer <key>` 且**逐字相等**（安全相关：若该头被静默丢弃，请求以未认证身份发出而零信号）；`api_key=None` ⇒ 必不带该头（闭世界另一半）；endpoint 单/多尾斜杠均被 `trim_end_matches('/')` 规范化 ⇒ 路径 `/search`（trim 全部而非只一个）。见 [ADR 0097](adr/0097-remote-request-construction-closed-world.md)、[W119 报告](reports/2026-10-16-w119-remote-request-construction-closed-world.md)。只钉出站侧事实，不断言远端真的接受凭证；endpoint 两条判别力弱于凭证头两条（若下层折叠 `//` 则仍会绿），定位为回归哨兵+语义记录。）
（W120 已把客户端内部**预算与边界**分支钉成可测判据：既有 429 用例只用过 `Retry-After: 0` 且总在预算耗尽前成功 ⇒ ①`backoff >= op.remaining()`（remote.rs:232-234，移除则硬等到期，结果从 `Unavailable` **变成 `Timeout`**）、②`attempt >= max_retries`（220-223，移除则退避 0 时**无限重试**）、③5xx 走 200ms 指数退避并**忽略** `Retry-After`（225-231；只有 429 才用 `Retry-After`，若 5xx 误用则 `Retry-After:100` 立刻撞上 ①⇒不再重试）、④`read_body` 8 MiB 上限（133-135，移除则无上限缓冲）四条分支**此前一次都没被执行**。新增 4 测试均用**判别性构造**（断言在分支被移除时必然变红），并把尝试次数显式清点；8 MiB 用例经**合法** `evidence[0].text` 撑大（畸形 JSON 也返回 `Invalid`，会与「读到大包被拒」混淆）。见 [ADR 0098](adr/0098-remote-retry-budget-closed-world.md)、[W120 报告](reports/2026-10-16-w120-remote-retry-budget-closed-world.md)。只钉可判事实（是否重试/何时放弃/放弃成什么），不断言真实网络退避时序与 `Retry-After` 语义正确性。）
（W121 换 crate 勘定：安全层 `Operation`（nai-security/src/lib.rs:30-73，所有带界操作共用的预算载体）的
**运行期终止语义**此前整段零判据——既有 in-src 测试只钉住**构造期**（`CEILING == 30s`、`new(CEILING).is_ok()`、
越界 `+1ms` 被拒）。新增 4 个 in-src 测试补齐：①`check()` **先判 cancel 再判 expiry**
（用 `Operation::new(Duration::ZERO)` + `sleep(20ms)` 构造「既取消又超期」⇒ MUST 报 `Cancelled` 而非
`DeadlineExceeded`；两条件同时为真 ⇒ 真正钉住优先序，而非只钉「cancel 生效」）；
②`remaining()` **单调不增**且超期为 0；③`stopped()` MUST **严格等于** `check().is_err()`
（在 live/cancelled/expired 三情形逐项核对 ⇒ 守卫不得自我满足、读取面不得与判定面分家）；
④`cancel()` 经 `Arc<AtomicBool>` **跨克隆共享**（`op.cancel()` ⇒ 克隆侧也 stopped）。
零生产改动；**不重复钉已钉住的构造期边界**。见 [ADR 0099](adr/0099-operation-budget-closed-world.md)、
[W121 报告](reports/2026-10-16-w121-operation-budget-closed-world.md)。）
（W122 转入**验收推进**：`tests/registry.yaml` 自 W112 起未被任何轮次改动 ⇒ 判据密度上升但**验收状态零推进**。
本轮把 `admission-registry` 早已声明的「`tested` 的证据 MUST 指向**真实报告**」补上载体——此前
`acceptance[].evidence` **零判据**（`grep evidence tests/registry.test.mjs` 只命中 `landing` 内容锚点），
故 `evidence` 可写空/写不存在路径/指向**未跟踪**文件而**门控全绿**。新增 4 条守卫遍历 registry 自己的键、
覆盖**全部 11 个 `tested` 项**：**非空 → 落在 `reports/` → 是仓库成员 → 磁盘存在**四步（判定顺序刻意如此，
**先判 tracked 再判磁盘**，否则断言随本机磁盘状态变化＝违铁律二）+ 闭世界覆盖与非空洞下限 + 两条注入反例。
⚠️ **实测教训**：registry 的 `evidence` 相对 **`docs/`**，而 `.git/index` 成员集相对**仓库根** ⇒ 判据必须先落
`docs/` 再判归属；首跑即因漏此而 11/11 假阳性 `not-tracked`。**状态回填只升真有证据者**：A13 升 `tested`
（静态结构事实，22 项断言在门控实跑）；**A03 维持 `implemented` 即使结构守卫已通过**——剩余缺口是外部真值，
「守卫通过 ≠ 本项达标」。语义不变量：`MET_STATUSES` 含 `implemented` 与 `tested` ⇒ 升级不改变 met/unmet 归类。
见 [ADR 0100](adr/0100-tested-evidence-verifiable.md)、[W122 报告](reports/2026-10-16-w122-tested-evidence-verifiable.md)。）
（W123 按建议啃 R1「本仓可闭合」项之首 **R10**：把**真实会话**接上升级激活门。`nai-runtime::Composition` 实现
`nai_upgrade_core::ActiveSessionGate`，读**它自己持有的** `Session`（核心刻意不持有会话状态——缓存会成为第二份
会过期的真相）。**语义（用户裁定 A）**：活跃 = **在播**（`Running`/`Draining`）；**不取**「一切未停止」，
否则本装配 `prepare` 即有的 `Created` 会让激活**永久 defer**，门恒挡＝死门。**不可观测 ⇒ fail closed**：
会话锁 busy/poisoned 返回错误，**MUST NOT 返回 0**（返回 0 等于谎称「无人活跃」，会让版本切换在开播中发生）。
判据 2 条（12→14）：真实 lifecycle Created→Preparing→Ready→Running（Deferred{active:1} + require_idle⇒
CapabilityNotReady）→Draining（仍 Deferred）→Stopped（Allowed）；busy 锁用**握手**造竞争 ⇒ Conflict（不靠 sleep）。
依赖方向：`nai-runtime → nai-upgrade-core` 合法（`runtime→shared`；反向 `shared→runtime` 被禁）。
**R10 仍不升 `tested`**：运行时**无升级激活调用点**（无 `UpgradeTransaction` 使用处）⇒ 命题尚未在真实链路执行，
按 ADR 0100「守卫通过 ≠ 本项达标」。见 [ADR 0101](adr/0101-runtime-session-gate.md)、
[W123 报告](reports/2026-10-16-w123-runtime-session-gate.md)。）
（W124 补 R08「存储维护有界」的**零覆盖上限**：实测 `MAX_PAYLOAD_BYTES`(64KiB) **0 引用**、快照拒绝
`-wal/-shm/-journal` 的 mutable journal 规则 **0**、`MAX_BYTES` **0**（遥测缓冲有 9 处 ⇒ 部分覆盖）。
新增 2 条判别性判据（55→57）：① 载荷 `cap/2` 通过、`cap+overhead` ⇒ **具名** `RESOURCE_EXHAUSTED`
（⇒ 静默截断也会红）；② 三个 journal sidecar 各注入一次 ⇒ `CONFLICT`，**且带对照**——删掉 sidecar 后
同一快照必须能恢复，否则「拒绝」可能只是备份坏了（假通过）。`MAX_PAYLOAD_BYTES` 改 `pub(crate)` 让判据
**引用**上限而非复制字面量（W77 对 `Operation::CEILING` 的同一条规矩）。
**R08 仍不升 `tested`**：256MiB 备份上限、publication / knowledge 上限仍零覆盖。
见 [ADR 0102](adr/0102-storage-bounds-carriers.md)、[W124 报告](reports/2026-10-16-w124-storage-bounds-carriers.md)。）
（W125 把 R08 从「四项点名」改成**有口径的名册**：按「具名拒绝（或具名丢弃计数）」普查 `crates/nai-storage`，
约束**单次写入或单份快照/备份可占用资源**的上限共 **19** 项，本轮前**只有 2 项有载体**——其余 17 项可被抬高
或删除而**零测试变红**。**口径 MUST 先定**：收**写路径**的字节/条目上限；**不收**读路径预算（检索扫描上限以
`truncated`⇒`RetrievalPartial` **报告**而非拒绝）与输入形状校验（id/token/`source_span`/`MAX_TOKENS_PER_CHUNK`/
`MAX_DIMENSION`/查询 token 与 `product_filters`——约束「什么算合法输入」，不约束「一次写入能有多大」）。
本轮覆盖 **8/19**（`nai-storage` **57→63**，+6 条判据）：**幅度判据**管「上限悄悄变大」、**边界判据**管
「闸门还在不在、边界在哪一侧」——夹具从常量派生 ⇒ 抬高常量**不会**让边界判据变红，缺一条就有一类漂移
无人看守；发布上限按**所选文档合计**判定（逐文档测试会把它判成通过）；快照读回两道闸门给出**不同**具名
拒绝（`snapshot file limit` vs `database hash/size mismatch`）⇒ 拒绝可归因；`MAX_ARTIFACT` **写入/读回**
两侧同值同界；**稀疏夹具** `set_len` 实测 0.39s 使 W124「256MiB 级夹具代价过高」的推迟理由作废。
**注入验证**：三条独立注入各自命中对应判据，幅度判据与 artifact 写入判据保持绿。
**R08 仍不升 `tested`**：名册 19 项**剩余 11 项零覆盖**（`store`/`budget`/`knowledge` 的调用与批上限，逐项具名在
registry）。见 [ADR 0103](adr/0103-storage-bound-roster.md)、[W125 报告](reports/2026-10-16-w125-storage-bound-roster.md)。）
（W128 把「开播中只暂存不激活」接到**生产调用点**上（R10）：`Composition` 新增 `stage_upgrade` / `activate_staged`，
`UpgradeTransaction` 在核心之外**首次**有使用处。三条形状规则：**暂存不问门**（暂存任何时候都安全、只有切换
需要静默 ⇒ 运行时不把两调合成一个方法，否则会把「被推迟」压成「失败」，而被门控的包恰恰不该失效）；
**门只被问一次**且由装配根**自己持有的真实会话**回答（两个方法都取 `&self` ⇒ 缓存活跃计数在**类型层面**不可能）；
**推迟是值、错误是错误**——门答 `Deferred{active}` ⇒ `UpgradeActivation::Deferred`（**非** `Err`），
门不可观测 ⇒ 原样上抛 `Conflict`（把「未知」写成推迟＝把未知伪装成已知的等待，写成放行＝让切换在开播中发生），
运行后的终局**原样**携带核心 `Outcome`（成因码 + 导出计数；运行时侧不重写、不补零）。
4 组注入逐条可归因（4／3／1／1 条红，I2 与 I3 的红色集合不相交）；一处断言**按构造**成立、无注入可令其
单独变红，已在测试注释如实标明。**前置**：`Prepared` 在核心之外**无法字面构造** ⇒ 判据必须走一次真的
`prepare` ⇒ demo 夹具从三份副本提为 `nai-upgrade-core/src/rehearsal.rs` 一处定义（另一个 crate 的 `tests/`
够不到）。**R10 仍不升 `tested`**：降档侧 `downgrade`/`require_rollback_compatible` 在核心之外**恰 0 个调用点**；
真实安装器与真实分发端点仍是外部注入点。见 [ADR 0106](adr/0106-runtime-upgrade-call-site.md)、
[W128 报告](reports/2026-10-16-w128-runtime-upgrade-call-site.md)。）
（W129 把 R08「存储维护有界」名册的**行为**那一半补齐并**升 `tested`**。W125 只把名册当**幅度**钉住
（常量是不是它自称的那个数），剩 11 项零覆盖——「零覆盖」的判据是 11 条拒绝名逐条全仓 `grep` 命中数**均为 1**
（只有定义处）⇒ 闸门可被删掉/改恒真/边界挪一格而零测试变红。三条方法论落点：①**同名数值不是同一个对象**
——三条 `65536`（配置文本／事实编码／候选编码）各有常量，共用一个会让「改其中一条」不知不觉改另外两条；
②**一个 `if` 里多条件 MUST 各自举证**——`store.rs` 候选路径的 `dependencies>32 || encode>65536` 拆出
`candidate payload limit`，**R5 注入（重合并）让载荷夹具拿到依赖拒绝名**即这条规矩的直接证据；
③**读窗口的 `+1` 是检测机制本身**（撤销 `LIMIT 4097` / artifact `LIMIT 513` 改为 `上限 + 1`），写死它等于
让「上限悄悄变大」时检测静默失效。**判据分工刻意保留**：夹具从常量派生 ⇒ 抬高常量**不会**让 8 条行为判据变红
（那是幅度判据的职责）；**R1 对照**（只动比较符/窗口、不动常量值）把名册幅度判据留绿，证明两者不互相顶替。
**名册闭世界**（第 9 条）：文件层／元素层三分是划分／反方向；谓词按「**顶格**」而非可见性筛——按可见性筛是
**弱类别**，私有上限（`MAX_DIMENSION`）会从后门溜走，首跑该守卫**抓到了它自己的作者**。
**诚实边界**：⑨ `MAX_LEDGER_ENTRY_BYTES` **今天从被认可的路径走不到**（`nai-budget-core` 把字符串限在
256 字符、`units` 限在 32 键 × 64 字符 ⇒ 最大合法条目仅几 KB）⇒ 具名登记、设计不上手改，配一条**会过期**的
可达性断言；本名册普查域**限于 `crates/nai-storage`**。**注入 8 组 / 17 条**逐条可归因 + 逐字节还原（7/7 OK），
全量 72 passed / 0 failed。**R08 升 `tested`**（缺口全清、无外部真值，判据见 ADR 0100 §6）。
见 [ADR 0107](adr/0107-storage-bound-behavior-coverage.md)、
[W129 报告](reports/2026-10-16-w129-storage-bound-roster-coverage.md)。）
（W130 把 S07「无明文密钥」的**诊断面**接上（口径含四面：仓库扫描／日志／诊断包／崩溃转储；W36 只建了仓库面）。
三条方法论落点：①**产物域与仓库域是两条扫描路径，缺省语义刻意不同**——仓库域对非 UTF-8 `else { continue; }`
**静默跳过**（具名登记的弱化，本轮**只加注释不修**），而产物域**必须收下非 UTF-8**：崩溃转储就是二进制，
`continue` 会把整条记录丢掉而报告上看与干净一模一样 ⇒ 有损解码后**照样扫**（ASCII 段逐字保留，高熵凭据恰在此），
并具名进 `non_text`。②**登记表是「产生者的名册」，不是「想扫的目录」的清单**——登记一个**没有产生者**的目录
等于给「扫了个空目录」**发通行证**（`present=true`、`scanned=0`，报告上看与干净无差）⇒ 本轮只登记 CI 门控日志，
诊断包与崩溃转储的「无产生者」**具名在册**而不是用空目录敷衍；登记凭据由一条判据**双向**钉住（`marker` 逐字存在于
`producer` 文本里、且点名 `dir`、且生产者是已跟踪文件、且**不是判据自身**）。③**`always()` 是必须的，不是保险**
——产物扫描判的是**前序步写的产物**，而 GitHub 默认「前序步失败 ⇒ 后续步跳过」⇒ 没写 `if:` 等于「门控一红，
扫描就消失」，一个**在需要它的那一刻才消失**的判定（落成 W100 的 **R6**；`always` 也因此成为条件形状表独立一格，
它与「没写条件」在**平台集合**上等价、差别**只在失败语义**）。**闭世界台账**：`visited` 与四桶（文本／非文本／
非文件／容器）**各自独立累加** ⇒「条目 == 四桶之和」是一条**可失败**的判据（A3 注入：`scanned` 改成派生即红）。
**规则与数据分开**：`always`/R6 是**规则**（写规格），新增成因取值 `ci-produced-artifact`、兑付格、`ARTIFACTS` 一行是
**数据**（W95/W108 的 Requirement 是生成的 ⇒ 不重复写，否则同一件事两处定义）。**注入 11 条**逐条可归因 +
逐字节还原（四文件 `sha256sum` 全等于基线）；B1 **只**触发 R6（R1/R2 未误报）、B1b 同时触发 R6 + R3；
`nai-xtask --lib` 52 → **60**、node 六文件 **110/110**。**门控 4 跑 2 绿**：composition（5s 期限 `Operation`
越限）与 engine `pipeline`（子进程被杀 ⇒ `status: None`）各红一次 ⇒ **失败集合跨目标漂移**、相关文件
（`crates/nai-runtime`/`crates/nai-storage`/`apps/engine`）`git status` 为空、单独重跑全绿 ⇒ 判为**宿主瞬态**；
**限并发只是减轻不是消除**（r3 正是在 `RUST_TEST_THREADS=1` 下红的），故结论不落在「限并发后全绿」上。⚠️ **「一条 Requirement 只声明一条行为」有可执行的落点，
但落点在归档之后**：`validate <change> --strict` 对 **MODIFIED 的正文长度豁免**（`validator.js` 第 255–258 行逐字
`MODIFIED is left alone`），而 500 字符上限只在**主规格**上查；`openspec validate` 又**不在**
`cargo xtask check --profile full` 的步骤集合里 ⇒ **超长的 MODIFIED 正文只在归档之后才被看见**（本轮实测
604 / 838 字符各报一次，两条 validate 从 43/0 掉到 41/2）⇒ 处置＝**就地重分 MODIFIED + ADDED 并重新归档**
（不压字、不动判据），并把「规格校验不在本机门控里、**归档前无信号**」具名登记（CI 的两步规格校验
——`foundation.yml:93` 的 `pnpm spec:validate` = `--all --strict`、94 行的 `--archived`——迟到但会红）。**S07 保持 `implemented`**（不升）——四面已建其二，
诊断包/崩溃转储在仓库内**无产生者** ⇒ 建不了，缺口仍在。
见 [ADR 0108](adr/0108-diagnostic-artifact-secret-scan.md)、
[W130 报告](reports/2026-10-17-w130-diagnostic-artifact-secret-scan.md)。）
（W131 把 S10 子句一「`local_only` 不外发」补上，起点是**registry notes 里的一个不实陈述**：
它逐字写「`local_only`/凭证边界已实现」，而字面 `local_only` 在全仓**零命中**——唯一出现它的地方就是
那一行自己。**教训：候选清单 MUST 逐轮现测，且「已实现」这种声明 MUST 回实现体核对，MUST NOT 由 notes
的字面推断**（本轮正是靠现测才发现 claims 与代码不一致；另两条子句现测**已建且真执行**，故范围只收
到子句一）。三条方法论落点：①**判定输入 MUST 从被断言的对象读出**——“哪些文档会离开本机” MUST 由
`documents.local_only` 读出，MUST NOT 由调用方当参数给（调用方给不了、也改不了那份上传范围）；
对应地**写入侧与读回侧都要注入**（注入 13：INSERT 的 `local_only` 换成常量 `0`，读回侧再正确也白搭）。
②**存疑取值选不放行那一边**：列是 `INTEGER`，SQLite 不限死 0/1 ⇒ 只有 `0` 是「未被标记」、其余一律按
**被标记**读（`!= 0`），注入 `== 1` 即红。③**永不🔥的守卫是装饰，不是判据**：可外发集合的规模已由
`MAX_SNAPSHOT_DOCUMENTS` 构造性界定 ⇒ 判定面**不**再造一个条数上限（那是同一件事的第二处定义，
而且域里永远到不了界）；同理**不**替 `sync` 再答一次「上传是否被允许」（同一开关答两次会漂移，W88/PD-31）。
⚠️ **W127「注入不变红 ≠ 注入坏了」的第二个方向——判据根本没有覆盖那一层**：本轮跑到
`JOIN knowledge_active_snapshots` 时发现**没有任何既有用例能走到那一层**（既有两次发布的用例里修订都变了，
旧快照行被 `s.revision=d.current_revision` 天然挡掉）⇒ 判据不是被更早的闸门短路，而是**没有输入能走到它**。
**处置＝补能走到那一层的输入**（两次发布且修订未变 / 两个 `kb_scope` 各有活动快照），**MUST NOT 改判据**；
补上后两条注入各红**不同**的用例 ⇒ 两条域判据互不顶替。**注入 13 条真跑、12 条真红**：注入 1（扣留判据失效）
与注入 2（静默丢弃）红的**断言层**不同（行 46 vs 行 41）⇒ 具名扣留与闭世界是两条独立判据。
⚠️ **注入验证的基线 MUST 是 fmt-clean 的最终字节**（W129 的坑本轮**再次**抓到：新加用例里的 `execute(...)`
需折行，`cargo fmt --all --check` 红 ⇒ 先修再取基线，否则 `sha256sum -c` 核对的是过渡态）。
**规格侧**：三条 ADDED 的正文**提前**自查为 268 / 250 / 106 字符（≤500），刻意**不 MODIFIED** 既有的
`Remote knowledge contract and revocation`（其正文已很长，加条款会越界）⇒ 避开 W130 那种「归档后才红」。
**registry `S10` 保持 `implemented`**——四环里①判定面②执行体已建且有覆盖，但③调用方**未闭合**
（`RemoteGateway::sync` 不消费这份清单、入参仍由调用方给）、④真实产生者**无**（`sync` 零生产调用点）
⇒ **没有任何代码路径会真的执行「按这份清单外发」**，与 W130 的 `S07` 同口径。**具名登记**：①`sync`
不消费 ⇒ 「外发一定经过这份清单」**没有**结构性保证；②老行读到 `0` 的含义是「**从未被标记**」而**不是**
「已核实可外发」（后者是没有人做过的核对，MUST NOT 写成前者的同义词）；③**TS 生成物内容漂移本机无判据**
（注入把 `backup.ts` 的版本号改回 9、`schema.test.mjs` 仍 **30/30 全绿**——那份判据只断言「文件存在与清单
对齐」，不比内容；只有 CI 的 `cargo xtask schema --check` 能抓，本机缺 protoc），与 W129 的同名缺口**同一处**。
见 [ADR 0109](adr/0109-local-only-egress.md)、
[W131 报告](reports/2026-10-17-w131-local-only-egress.md)。）
（W132 补上 H17 的 **runtime 接线**。起点是一个**判定面答不了的问题**：`DegradeVerdict::visible` 是
`evaluate_degradation` 无条件写入的 `true` ⇒ 判定面**没有「不可见」这一格**，于是「无静默降级」钉在
判定面**恒绿、是装饰**；真正会坏的是**判定到界面的 transport**，本轮交付的正是它（渲染 → 落盘 → 读回）。
三条方法论落点：①**构造性事实不能用断言去守，但要把判据搬去它真正能坏的那一跳**——`visible` 恒真不是
缺陷，可它意味着判据 MUST 落在 transport 上（不做「再断言一次 visible」这种装饰）。②**入参是事实、
不是判定**：`record_degradation` 收「哪些层失败／还剩什么可用／上下文还留着什么」，**不**收
「落到哪一级」——后者由 `evaluate_degradation` 在生产路径上算出；收判定结果就是**注入判定**，
runtime 侧将不再有任何东西保证这个映射（与 W131 的「判定输入 MUST 从存储读出」同族，但方向相反：
那一条是**读**不能由调用方给，这一条是**判**不能由调用方给）。③**「注入不变红」有第三种成因**：
本轮 R2 首跑**假绿**，因为写成 `!cfg!(test)` —— 而 `cfg!(test)` 在**生产 lib**（`degradation.rs`
不是测试 crate）里恒为 `false`，注入**没改到任何行为**。⇒ 「注入后全绿」MUST **先怀疑注入本身**，
MUST NOT 直接记作「判据无判别力」。**再实证 ADR 0109 的第二个方向（K4）**：写「层首选级」而非
「回退后落点级」的坏实现，让 kernel **29/29 全绿**、runtime **既有 8 条全绿**——它们**全部**用
`DegradeContext::all_available()`，在该输入下首选级恒等于回退级，**没有任何既有输入能走到那一层**；
只有本轮新补的 `a_failure_whose_preferred_level_is_unavailable_shows_the_level_it_walked_down_to`
（`template_available:false, approved_cache_available:false, human_available:true`）会红。
**处置＝补能走到那一层的输入，MUST NOT 改判据**。⚠️ **不扩展 `CapabilityView` 契约**：
`CapabilityDescriptor` 描述的是**插件能力**（`capability_id`/`plugin_id`/`PluginState`），把 harness 的
层失败映射进去要编造这些字段且 `PluginState` 无对应取值 ⇒ 那是在说「一个插件失败了」，而事实是
「某一层失败了」。**不实陈述比缺口更贵** ⇒ 具名登记为仍未接线（另需 protoc 3.21.12 才能改 proto，本机无）。
**`degradation` 判 critical**：telemetry 写失败会落进**遥测缓冲**、调用方只看到 `persisted == false`
⇒ 一条能悄悄消失的降级正是静默降级；代价（写入失败变硬错误、暴露在热路径）是刻意付的。
**registry `H17` 仍 `pending`**——四环里①②已建且有覆盖，③调用方（呈现层渲染）与④真实产生者未接线；
本轮交付的是「降级项清单**存在且可被界面读到**」，**不是**「界面已经显示了它」（与 H15/H01/H02
接线轮同口径）。见 [ADR 0110](adr/0110-degradation-visible.md)、
[W132 报告](reports/2026-10-17-w132-degradation-visible.md)。）
（W133 补上 H01/H02 的**缺失生产者**。起点不是「某条规则没实现」，而是**两端之间是空的**：现测
`grep -rn AssemblyPlan` 排除测试后只有定义与消费两处——生产代码里**没有任何一行构造计划**，
`record_assembly` 的入参从未被生产过，仓库里每一份计划都诞生在测试里、也死在测试里
⇒ **「已接线」可以描述一个没有调用方的函数体**（W107/W110 之后同一类措辞陷阱的第三次）。
装配器只决定三件事：施加 §41.1 的顺序（不回显调用方输入序）、只输出被提供的段
（**绝不补空切片**——补了会让 `UncroppableSegmentMissing` 在真实路径上**没有对象可拒**）、
段内从**尾部**裁剪并同时记录被裁项与原因；1–3 段任何预算都不裁，超预算交判定面拒。
**零业务常量**：§41.2 的初值必须由 §47.2 黄金集回放标定并记 ADR，在这里发明一个数字等于把 §41.2
答第二次。⚠️ **分组 MUST 按「缺口是否同型」而非按编号区间**：原计划「H01–H04 一组」在勘定后拆掉——
H01/H02 缺一个**纯计算**的构造者，H03/H04 缺「对话轮次产生记忆候选」（需模型输出解析，与 H16 同源），
硬凑会逼出「四条都推进了」这类不实陈述。**registry `H01`/`H02` 仍 `pending`**（闭合的是②执行体
获得真实入参，④真实产生者与 §41.2 初值标定未接线）。见
[W133 报告](reports/2026-10-17-w133-assembler.md)。）
（W134 补的是**执行面上的重复**。起点是「CI 太慢」这句抱怨，现测之后结论不是慢，而是**同一件事做两遍**：
`foundation.yml` 在 `check --profile full`（内含 `cargo test --workspace`）之后又跑了 7 条
`cargo xtask test --suite …` 步，包集全部是那次全量运行的**子集**——本机暖缓存实测这 7 条合计
**517 秒纯执行**（第二遍），CI runner 只会更慢。值得写下来的是：**四层既有判据都在，四层都不问这件事**
（W95 步集闭世界／W97 覆盖闭包／W100 执行配置／W101 目标绑定都正确，都对「两个目标覆盖的是不是
同一批测试」沉默）⇒ **重复是它们共同的盲区**。补法是把「覆盖关系之间」变成判定对象：两个判定步的覆盖
集合互相包含（含相等）即具名报错，包集从 dispatch 臂与门控区域**算**出来（MUST NOT 写第二份）。
⚠️ 两条方法论：①`suite` 类里「体是一条 `["test", …]`」与「体交给别的函数」（`contracts` 的 C++ 夹具）
MUST 分开——后者返回 `null` 而非空集，空集会让包含关系对空集恒真；②删步会让一批既有判据的
`replace` 锚点变成**静默 no-op**（注入没发生、断言前提没了 ⇒ 「注入后应当红」的用例反过来全红），
锚点 MUST 改到**单行 `- name:`** 的幸存步并补「锚点 MUST 命中」的现测。另实测：`ciXtaskCommands`
**不剥 YAML 注释**，注释里连写 `cargo`+`xtask` 会被当成一条真实 CI 命令（本轮只避开，洞未修）。见
[W134 报告](reports/2026-10-17-w134-ci-step-coverage.md)。）
（W135 补的是**时间的盲区**：W134 修了「漏登记的那一个文件」，没修「漏登记」这一类。提交集 =
索引集 ∪ 补集，而 W84 的闭世界**只读索引**（`resolveSite` 刻意不用 `readdir`——未跟踪 ⇒ 尚未承诺为
仓库内容 ⇒ 不能充当覆盖证据，W74/W75）；可 §5 的暂存命令是 `git add -A`，它会把补集收进提交
⇒ 未登记的测试文件**提交前对判据结构性不可见、提交后立刻可见**：本机全绿、CI 一 checkout 就红，
且红在一条与该文件无关的断言上。补法是**正交加法**：新读取器只读补集（`git ls-files --others
--exclude-standard`），并断言 `present ∩ tracked == ∅`，两条判据合起来恰好覆盖提交集一次；
**MUST NOT** 把 `resolveSite` 的输入换成工作区——那会让**未跟踪的伪造消费者**重新进入覆盖集，
正是 W74 F2 要修的东西。⚠️ 三条方法论：①**「不可解析 ⇒ 具名失败」在新增读取器上同样成立**——
派生失败若返回空集，就把「不知道」写成了「没有」，这正是本轮的缺陷形状；②**非空洞要两条前提**：
`scanned > 0` 只证明输入非空，**不证明清扫谓词不惰性**（注入实测：关掉 `candidates.push` 后真实仓库
用例与 F0 同时绿，只有喂具名路径的单元用例会红）⇒ 合成输入前提 MUST 钉在真实仓库用例里；
③**命名会撞上安全扫描**：`xtask/src/secrets.rs` 的 `sk-` 前缀规则 `min_len: 32` 会把
`xtask-engineering-pending-registration`（`sk-` 后恰好 32 字符）判成 OpenAI 凭据 ⇒ 处置是**命名避让**
（id 改短），MUST NOT 加豁免削弱判据。见 [W135 报告](reports/2026-10-17-w135-pending-registration.md)。）
响应为 `deny_unknown_fields` 的有界 DTO，坏响应返回 Invalid，不静默变 empty。
`rerank_evidence` 只重排已合法证据 ID，未知 ID 丢弃；子预算耗尽保留 base 为 partial，整体取消/超时停止。
`Store::remote_delete_targets` 返回本地 tombstone 后的 kb-scoped 删除目标；远端失败不回滚本地失效。

## 检查与剩余任务

```bash
cargo xtask test --suite knowledge
cargo xtask check --profile portable
cargo xtask test --suite contracts
```

最新 knowledge suite **55 项通过**（算法 11、remote mock 10、storage 34）；portable **157 项 Rust 测试通过**。
发布/恢复详情见 [发布报告](reports/2026-10-08-d09-knowledge-publication.md)。
真实远端端点/凭据缺失，付费 embedding 受 D08 无余额证据约束，真实调用为 Blocked；mock 不标真实集成为 Passed。
文件格式解析器/OCR、完整导入 UI 和媒体 preplay 装配不由这些存储/网关方法冒充实现。
D09 已满足本规格并归档；真实供应商接入仍需有效凭据与独立认证。
