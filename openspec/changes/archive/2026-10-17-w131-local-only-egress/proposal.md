# Proposal: W131 — S10 子句一「`local_only` 不外发」

## Why
`tests/registry.yaml` 的 `S10` notes 逐字写着「**`local_only`/凭证边界已实现**；长期画像未建」。
本轮开工前**现测**：字面 `local_only` 在 `crates/`、`apps/`、`xtask/`、`scripts/`、`tests/` 里
**零命中**——唯一出现它的地方就是 registry 这一行自己。也就是说 notes 里的「已实现」是一个
**可判事实的不实陈述**：它没有实现体。

基线 `docs/直播数字人 + 智能体系统技术方案 V3.11.md` 里 `local_only` 出现 4 次，口径是一致的：
- 行 **89**（INV-17）：「数据外发范围……受 `local_only` 约束」
- §**281**：「标记 `local_only` 的文档**只走本地词法检索或人工**」
- §**563**：「上传范围」受 `local_only` 约束
- §**13.2**：「`local_only` 数据不上传」

`S10` 的验收口径是三条子句，本轮逐条现测：

| 子句 | 现测结果 | 证据 |
|---|---|---|
| ① `local_only` 不外发 | **零落点** | 全仓 `grep -rn local_only` 只命中 registry 自己 |
| ② 画像默认关闭 | **已建** | `MemoryPolicy` 六项（`viewer_memory_enabled:false`/`allowed_viewer_scopes`/`allowed_purposes`/`allowed_viewer_fields`/`max_retention_ms`/`forbidden_sensitive`）在 `admit_write` 里**真执行**（`evaluate/memory.rs` 行 189–222） |
| ③ 撤销与删除按期限清除 | **已建** | `ForgetTrigger::ALL` 含 `Ttl`；`revoke_knowledge` + `purge_memory_body` 已接线 |

⇒ 本轮**只补子句一**；②③不动，也不据此改它们的判定。

## What Changes
1. **承载物**：新增 `migrations/sqlite/0010-document-local-only.sql`
   （`ALTER TABLE documents ADD COLUMN local_only INTEGER NOT NULL DEFAULT 0`），
   并同步版本号成套**六处**：`migration::VERSION` / `SQL` 数组 / `backup.schema.json` 的 const /
   Rust 契约样本断言 / `tests/contracts/schema.test.mjs` 备份样本（含「上一版 MUST 被拒」+1）/
   TS 生成物 `packages/contracts-ts/src/generated/config/backup.ts`。
   ⚠️ 老行读到 `0` 的含义是「**从未被标记**」，**不是**「已核实这份文档可以外发」——后者是没有人做过的核对，
   MUST NOT 被写成前者的同义词。这一点写进迁移文件注释、写进用例、也写进报告。
2. **导入入口**：`ImportedDocument.local_only` 是**必填**（无默认值）⇒ 每一处构造都必须表态，
   「忘了给」在编译期就过不去，MUST NOT 依赖运行期兜底。
3. **判定面**（`crates/nai-knowledge-core/src/egress.rs`）：`EgressCandidate` / `EgressPlan` /
   `plan_egress`。两桶闭世界（元素**恰好**是输入）+ 具名扣留（稳定标记 `local_only`，不是缺席）+
   判定单元是 `(document_id, revision)` 这个**身份**而不是名字（重复身份**被拒**，不是被计数两次）+
   空集报 `0`（不短路成跳过）。
4. **执行体**（`Store::remote_egress_targets`）：域 = **活动快照 ∩ 未撤销 ∩ 该 `kb_scope`**；
   标记从 `documents.local_only` **读出**，MUST NOT 由调用方给。列上只有 `0` 是「未被标记」，
   任何其它值按**被标记**读——外发是危险方向，存疑时选不放行那一边。
5. **刻意不做**（三条，理由写进 ADR 0109）：
   - **不再回答「上传是否被允许」**：那是 `RemoteGateway::sync` 的 `allow_document_upload` 判定。
     同一个能力开关答两次会漂移（W88 / PD-31），所以本面**没有** policy 参数。
   - **不引入新的条数上限**：可外发集合的规模由快照自己的名册上限界定
     （`MAX_SNAPSHOT_DOCUMENTS`），这里再造一个数就是同一件事的**第二处定义**，而且那个闸门
     **不可能开火**（域里永远到不了界）——一个永远开不了火的守卫是装饰，不是判据。
   - **不动 `sync` 的入参**：`sync` 仍收调用方给的清单（具名登记，见下）。

## 具名登记（本轮**发现但不修**的弱化，MUST NOT 顺手处理）
1. **链条的最后一环未闭合**：`remote_egress_targets` 是一份**清单生产器**，`RemoteGateway::sync`
   的入参**没有**被强制接它 ⇒ 「外发一定经过这份清单」这件事**没有**结构性保证。本轮只闭合了
   「仓库里存在一份按存储读出的清单」。⚠️ 另：`sync` 在仓库里**零生产调用点**（只有测试调用），
   与 d09 报告「真实远端 Blocked」一致。
2. **写入侧的标记改不了老行**：`0010` 是 `NOT NULL DEFAULT 0`，历史文档一律读到「未被标记」。
   这不是缺陷（它是升级时点的历史事实），但 MUST NOT 被读成「已核实可外发」。
3. **TS 生成物的内容漂移本机无判据**：本轮注入把 `backup.ts` 的 `db_schema_version` 改回 9，
   `node --test tests/contracts/schema.test.mjs` **30/30 全绿**——那份判据只断言「文件存在与清单对齐」，
   不比内容。能抓它的只有 CI 的 `cargo xtask schema --check`（本机缺 protoc 3.21.12，跑不了）。
   这与 W129 已登记的同名缺口是**同一处**，本轮再确认一次，不新增判据。

## Modified Capabilities
- `knowledge-pipeline`：**ADDED** 三条 Requirement（正文 268 / 250 / 106 字符，均 ≤ 500）。
  刻意**不 MODIFIED** 既有的 `Remote knowledge contract and revocation`——它的正文已经很长，
  往里加条款会越过 500 字符上限（W130 实测：归档后 `validate --all --strict` 才报），且
  「一条 Requirement 只声明一条行为」。三条各自声明一件事：
  - `A document marked local-only is withheld from the release list by name`（判定面）
  - `The release list is read from storage rather than supplied by the caller`（执行体与域）
  - `Withholding is not removal from local retrieval`（正向半：不外发 ≠ 不可检索）
