# Design: W125 — 存储写入上限的名册与快照/发布族覆盖

## 名册（闭世界，本轮冻结）
约束「单次写入或单份快照/备份可占用资源」的上限，按 `nai-storage` 的**具名拒绝**
（或具名丢弃计数）普查，共 **19** 项：

| # | 上限 | 位置 | 具名拒绝 | 本轮前 |
|---|---|---|---|---|
| 1 | `decision::MAX_PAYLOAD_BYTES` = 65536 | `decision.rs:171` | `decision payload too large` | ✅ W124 |
| 2 | `decision::TELEMETRY_BUFFER_CAPACITY` = 1024 | `decision.rs:326` | `dropped_oldest` 计数 | ✅ |
| 3 | `knowledge::MAX_CONTENT_BYTES` = 16MiB | `knowledge.rs:207/244/245` | `document exceeds import bound` | ✗ → ✅ |
| 4 | `knowledge::MAX_CHUNKS` = 4096 | `knowledge.rs:219/323` | `invalid chunk count` | ✗ → ✅ |
| 5 | 撤销批次 4096 | `knowledge.rs:411` | `revocation revision bound` | ✗ |
| 6 | `publication::MAX_CHUNKS` = 4096 | `publication.rs:166` | `snapshot chunk count bound` | ✗ → ✅ |
| 7 | `publication::MAX_BYTES` = 16MiB | `publication.rs:166/170/250/259` | `snapshot chunk byte bound` | ✗ → ✅ |
| 8 | 快照文档数 256 | `publication.rs:140` | `invalid snapshot documents` | ✗ |
| 9 | `snapshot::MAX_BYTES` = 256MiB | `snapshot.rs:237/249/286/331` | `snapshot file limit` | ✗ → ✅ |
| 10 | `snapshot::MAX_ARTIFACT` = 16MiB | `snapshot.rs:151/228/329` | `artifact type/size invalid` | ✗ → ✅ |
| 11 | 托管文件数 512 | `snapshot.rs:130` | `snapshot artifact count limit` | ✗ |
| 12 | 配置文本 65536 | `store.rs:288` | `configuration too large` | ✗ |
| 13 | 事实编码 65536 | `store.rs:497` | `fact too large` | ✗ |
| 14 | 候选编码 65536 / 依赖 32 | `store.rs:563` | `candidate dependency limit` | ✗ |
| 15 | 待审批容量 200 | `store.rs:584` | `session pending approval capacity` | ✗ |
| 16 | 候选证据引用 32 | `knowledge.rs:114` | `candidate evidence bound` | ✗ |
| 17 | 账本条目编码 65536 | `budget.rs:26` | `oversized ledger entry` | ✗ |
| 18 | 已存账本文本 132000 | `budget.rs:33` | `oversized stored ledger` | ✗ |
| 19 | 十六进制载荷 131072 | `budget.rs:48` | `invalid stored ledger bytes` | ✗ |

**本轮覆盖 8 / 19**（1–4、6、7、9、10，其中 1 由 W124 落地），剩余 **11** 项具名在册
⇒ 覆盖仍属部分，`R08` **不升 `tested`**。

## 口径（MUST 明写，否则名册是「按感觉圈」）
本项收的是**写路径的资源上限**。两类**不收**，各有理由：
- **读路径预算**：`knowledge.rs` 的检索扫描上限 4096 以 `truncated` ⇒ `RetrievalPartial` **报告**而不是拒绝。
  它约束一次查询的代价，不是可持久化资源的增长；拒绝语义与「有界存储」不同，故不并入。
- **输入形状校验**：`lib.rs` 标识符 ≤256、`upgrade.rs` ≤512、token ≤64、`source_span` ≤256、
  `MAX_TOKENS_PER_CHUNK`、`MAX_DIMENSION`、查询 token ≤64 / `product_filters` ≤64。
  它们约束「什么算合法输入」，不约束「一次写入能有多大」；尤其 `MAX_DIMENSION` 是索引**身份**约束
  （同一 index 的向量必须同维度），与资源增长无关。

## 判据设计
1. **名册幅度判据**（1 条）：逐项断言常量等于声明幅度。它管**幅度漂移**；边界判据管**行为**。
   这个分工是刻意的：夹具从常量派生 ⇒ 抬高常量**不会**让边界判据变红，只有幅度判据会。
   没有它，「上限」可以无声地变成任意大而所有边界判据照旧全绿。
2. **双侧边界**：导入内容/分块数、artifact 单份大小——**恰在界上必须通过、越一字节/一块必须具名拒绝**。
3. **聚合判据**：`publication` 的上限按**所选文档合计**判定 ⇒ 两份各自合法的文档合起来必须被拒
   （逐文档测试会把它判成通过）。
4. **归因对照**：快照读回的两道闸门给出**不同**具名拒绝——`snapshot file limit`（超限）与
   `database hash/size mismatch`（同尺寸改字节）——所以「太大」不会被读成「损坏」，反之亦然。
5. **稀疏夹具**：`checked_file` 先比 `metadata().len()` 再读，故 `set_len` 造逻辑长度即可到达尺寸闸门
   （实测 0.39s / 逻辑长度 256MiB+1），不必真写 256MiB。
   W124 曾以「256MiB 级夹具代价过高」为由推迟本项，该理由由此作废。

## 明确不做
- 不覆盖剩余 11 项：覆盖是**逐项**的，每项都要一条可归因的夹具，不能靠「同一族」推断。
- 不把读路径预算与输入形状校验并入本项（见口径）。
- 不改任何上限的**取值**，也不改拒绝的 `ErrorCode`；只把一处**合并理由**拆成两条。
