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
