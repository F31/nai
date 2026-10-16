# Design: W129 — 存储上限名册的行为覆盖

## 缺口（逐项现测，MUST NOT 由「同族」推断）
W125 普查所得 19 项里，本轮开工前**零覆盖**的是下列 **11 项（12 个条件）**。
「零覆盖」的判据是：全仓搜该拒绝名，只有**定义处**一处命中（`grep` 计数 = 1 × 11）。

| # | 上限 | 承载常量（本轮新建） | 承载模块 | 具名拒绝 |
|---|---|---|---|---|
| 1 | 撤销批次 = 4096 个不重复 `(document, revision)` | `knowledge::MAX_REVOCATION_REVISIONS` | `knowledge.rs` | `revocation revision bound` |
| 2 | 快照文档数 = 256 | `publication::MAX_SNAPSHOT_DOCUMENTS` | `publication.rs` | `invalid snapshot documents`（空选集同名的另一分支） |
| 3 | 托管文件数 = 512（按不重复路径） | `snapshot::MAX_SNAPSHOT_ARTIFACTS` | `snapshot.rs` | `snapshot artifact count limit` |
| 4 | 配置文本 = 65536 | `store::MAX_CONFIGURATION_BYTES` | `store.rs` | `configuration too large` |
| 5 | 事实编码 = 65536 | `store::MAX_FACT_BYTES` | `store.rs` | `fact too large` |
| 6a | 候选依赖条数 = 32 | `store::MAX_CANDIDATE_DEPENDENCIES` | `store.rs` | `candidate dependency limit` |
| 6b | 候选编码 = 65536 | `store::MAX_CANDIDATE_BYTES` | `store.rs` | `candidate payload limit`（本轮**新增**；原先并入 6a） |
| 7 | 待审批容量 = 200 | `store::MAX_PENDING_CANDIDATES` | `store.rs` | `session pending approval capacity` |
| 8 | 候选证据引用 = 32 | `knowledge::MAX_EVIDENCE_REFS` | `knowledge.rs` | `candidate evidence bound` |
| 9 | 账本条目编码 = 65536 | `budget::MAX_LEDGER_ENTRY_BYTES` | `budget.rs` | `oversized ledger entry` |
| 10 | 已存账本文本 = 132000 | `budget::MAX_STORED_LEDGER_BYTES` | `budget.rs` | `oversized stored ledger` |
| 11 | 十六进制窗口 = 131072 | `budget::MAX_STORED_LEDGER_HEX` | `budget.rs` | `invalid stored ledger bytes` |

## 两类判据的分工（刻意的，MUST 都保留）
- **幅度判据**（W125）：`the_declared_storage_write_bounds_are_the_magnitudes_the_item_claims`
  逐项断言「常量 == 自称的数」。它管**上限悄悄变大**。
- **行为判据**（本轮）：`tests/storage_bounds.rs` 的 8 条。夹具**从常量派生** ⇒ 抬高常量**不会**
  让它变红。它管**闸门还在不在、边界在哪一侧**。
这个分工是本轮设计里唯一不能省的：只有幅度钉，则「声明之后从不读取」的上限会全绿；
只有行为钉，则「上限从 4096 悄悄变成 40960」也全绿。两者缺一，名册就是纸面声明。

## 一组的关键构造决定
1. **界上那份必须真的恰在界上**。④ 用 `plugin_catalog`（schema 里唯一的自由字符串）把 JSON 顶到
   恰好 `cap`；⑤ 用「单调滚动定界」找最大的 `pad` 使编码恰好 `cap`，并**断言** `at_cap == cap`
   与 `over_cap > cap`。⑤ 的夹具是 `publish_fact` 三处改写（`revision` / `publisher_ref` /
   `permission_revision`）的**不动点**，且用例**断言**「落库字节 == 夹具测量」——镜像一旦跟生产
   代码分叉，用例会红，而不是悄悄测错东西（这是本轮修正过一次的地方：镜像把
   `workspace.permission_revision` 读成了 `String`，实测是 `INTEGER` ⇒ 夹具红，随即按生产路径订正）。
2. **一个 `if` 里的多个条件各自举证**。② 的空选集与越界共用一条拒绝名（两个分支）⇒ 两个分支各自
   给夹具；且越界那一侧故意混进一个**不存在**的文档，好让拒绝**只能**来自条数——不存在走的是
   另一条具名拒绝（有探针先确认它是另一条）。⑪ 的三条件（奇长度 / 超长 / 非十六进制）逐个举证。
3. **「恰能走到它那一层」**。⑪ 的越界夹具必须先断言 `len() < text_cap`，证明它没被 ⑩ 短路；
   ① 的 4097 夹具要先断言 `distinct_revisions == cap + 1`，证明它真的到了那一层。
   若不做这一步，注入该断言时得到的「全绿」只证明它没被触发，不证明它会拦（W127）。
4. **① 是本组唯一需要真实路径的**。表里的不重复 `(document, revision)` 只能由「重新导入（铸新版本）
   → 暂存快照」产生，一次一对。凑到界上是 16 轮 × 256 份文档 = 4096 对；界上那一次必须**过得去**，
   第 4097 对才被拒。`revoke_knowledge` 收尾时**不删** `knowledge_snapshot_documents` 行 ⇒
   「界上放行」与「越界拒绝」可用**同一夹具**先后验证。

## 一处具名登记（发现的弱化，MUST 具名登记，MUST NOT 顺手改）
`budget::MAX_LEDGER_ENTRY_BYTES = 65536`（⑨）**从被认可的路径走不到**：`nai-budget-core` 的校验
把每个字符串字段限在 **256 字符**（`id()`）、`units`/`concurrent` 限在 **32 个键 × 64 字符**
（`counters()`），且每个 unit 键 MUST 在三处限额（global/session/provider）里声明过（否则 `UnknownUnit`）。
于是最大的合法条目编码出来只有几 KB。设计**不上手改**（那是另一件事），但「它今天走不到」既被
**说清楚**，又被一条**会过期**的断言钉住：`maximal.encode_to_vec().len() < cap / 4`。
⑨ 的闸门本身仍要两侧都在——直接问 `budget::encode`（**接缝级**，MUST 如实标明不是路径级）。

## 注入验证矩阵（真跑，每条注入都确认「对应判据变红且可归因」，随后逐字节还原）
| 注入 | 归属 | 预期红的位置 |
|---|---|---|
| ④ `>` → `>=` | test1 | 界上 `save_config` 返回 `configuration too large` |
| ⑦ `>=` → `>` | test3 | 越界那条候选被接纳（`.unwrap_err()` on `Ok`） |
| ⑧ `>` → `>=` | test2 | 正对照（界上 32 条证据）被拒 `candidate evidence bound` |
| ③ 窗口去 `+1` | test5 | 513 条路径不再触发（`.unwrap_err()` on `Ok`） |
| ⑨ `>` → `>=` | test6 | `encode(界上)` 失败 |
| ⑩ `>` → `>=` | test7 | 界上文本读不回 |
| ⑤ `>` → `>=` | test1 | 界上事实被拒 `fact too large` |
| ② 计数 `>` → `>=` | test4 | 界上 256 份文档被拒 |
| ⑥ 依赖分支整体删除 | test2 | 拒绝换成后续闸门的 `fact absent or expired` |
| ② 空选集分支删除 | test4 | 空选集掉到下游 `snapshot chunk count bound` |
| ⑥ 载荷分支删除 | test2 | 越界载荷被接纳（`.unwrap_err()` on `Ok`） |
| ⑪ 长度条件删除 | test7 | 越界 hex 落到 `invalid stored ledger protobuf` |
| ⑥ 重合并为单 `if` 单名 | test2 | 载荷夹具拿到 `candidate dependency limit` |
| ① 窗口去 `+1` | test8 | 第 4097 对静默放行 `Ok(256)` |
| ① `>` → `>=` | test8 | 界上 4096 批次被拒 |
| 名册幅度（声明值改 65535） | test9 | `store::MAX_FACT_BYTES` 幅值不符 |
| 新增未归类顶格常量 | test10 | 闭世界报 `["store::MAX_PROBE_BYTES"]` |
**对照**：R1（六条只动比较符/窗口、不动常量值）把名册判据**留绿**——证明幅度钉与行为钉分工成立。

## 明确不做
- 不改任何上限的**取值**，不改拒绝的 `ErrorCode`；只把一处**合并理由**拆成两条。
- 不给 ⑨ 修校验放宽（那是设计变更，不是覆盖）。弱化只**具名登记**。
- 不把「读路径预算」与「输入形状校验」并入 R08（沿用 W125 口径，`SHAPE_LIMITS` 具名在册）。
