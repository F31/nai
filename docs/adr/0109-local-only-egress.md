# ADR 0109：S10 子句一——`local_only` 不外发，且判定输入 MUST 从存储读出

- 状态：Accepted｜日期：2026-10-17｜归属：W131（S10 子句一补建，**保持 `implemented`**）
- 关联：ADR 0092（W114 远端知识契约）、ADR 0078（W100 执行配置面）、
  `knowledge-pipeline` spec、`tests/registry.yaml` 的 `S10`、
  `docs/直播数字人 + 智能体系统技术方案 V3.11.md` 行 89（INV-17）/ §281 / §563 / §13.2

## 背景
`tests/registry.yaml` 的 `S10`（「数据外发与画像」）notes 此前逐字写着
「**`local_only`/凭证边界已实现**；长期画像未建」。开工前**现测**：字面 `local_only` 在
`crates/`、`apps/`、`xtask/`、`scripts/`、`tests/` 里**零命中**——唯一出现它的地方就是 registry
这一行自己。这是一条**可判事实的不实陈述**：它没有实现体。

基线里 `local_only` 出现 4 次（INV-17、§13.2、§281、§563），口径一致：标记 `local_only` 的文档
**不上传**，只走本地词法检索或人工。而 `S10` 的验收口径是三条子句，逐条现测后另两条**已建**：

- 子句二（画像默认关闭）：`MemoryPolicy` 六项（`viewer_memory_enabled:false`、
  `allowed_viewer_scopes`、`allowed_purposes`、`allowed_viewer_fields`、`max_retention_ms`、
  `forbidden_sensitive`）在 `admit_write` 里**真执行**（`evaluate/memory.rs` 行 189–222），
  不是只声明。
- 子句三（撤销与删除按期限清除）：`ForgetTrigger::ALL` 含 `Ttl`；`revoke_knowledge`
  与 `purge_memory_body` 已接线。

⇒ 本轮范围**只有子句一**；②③ 不动，也不据此改它们的判定。

## 决策

1. **标记落在文档级**，不是 kb / 会话 / 全局开关。§281 的原文主语是「标记 `local_only` 的**文档**」；
   做成全局开关就答不出「哪一份不能外发」，清单也就无从谈起。

2. **独立的列**（`documents.local_only`），不复用 `acl_ref` 或 `product_scope`。那是两个不同的事实
   （访问控制引用、产品域）。把 `local_only` 塞进其中任何一个，就是让一格记两件事——读的人无法分辨
   手里那个值说的是哪一种。同一件事 MUST NOT 两处定义，两个不同的事 MUST NOT 挤进同一个字段。

3. **`NOT NULL DEFAULT 0`，不是可空三态**。`local_only` 是一个**二值事实**（是 / 否），没有「未知」
   这一格；导入入口 `ImportedDocument.local_only` **必填**且无默认，所以每一条新写入都必须表态，
   「忘了给」在编译期就过不去，MUST NOT 依赖运行期兜底。可空会把「未标记」与「不知道」混为一谈，
   而后者在这里不存在。

4. **不改 `0001-foundation.sql`**。`verify()` 逐文件核对已应用迁移的 SHA-256，改 0001 会让每一个
   既有库的校验和漂移。与 0008 / 0009 同一个理由。

5. **判定面不带 policy 参数**。「文档上传是否被允许」是 `RemoteGateway::sync` 的
   `allow_document_upload` 判定。同一个能力开关答两次会漂移（W88 / PD-31：读取面 / 写入面 / 准入面
   共用一处判定），所以 `plan_egress` **只回答标记问题**，不替 `sync` 再答一次策略问题。
   它因此**没有** policy 入参——这不是残缺，是刻意的边界。

6. **不引入新的条数上限**。可外发集合的规模由快照自己的名册上限界定（`MAX_SNAPSHOT_DOCUMENTS`），
   这里再造一个数就是同一件事的**第二处定义**，而且那个闸门**不可能开火**（域里永远到不了界）。
   一个永远开不了火的守卫是装饰，不是判据。（另：`remote.rs` 里的 `16` 已硬编码两处——`sync` 行 642 /
   `delete` 行 671——同形不同义，本轮**不动**，避免把「同一件事两处定义」从两处变成三处。）

7. **扣留是带稳定标记的桶，不是缺席**。被扣留的文档如果只是「不在可外发清单里」，清单看起来只是更短，
   没人知道少了什么、为什么少。`EgressWithholding::LocalOnly` 因此是一个**具名值**，进日志、进审计、
   进判据的输出比对。

8. **判定单元是 `(document_id, revision)`，不是 `document_id`**。同一个文档的两个修订是两份东西；
   「同名」≠「同一个」。重复身份**被拒**（`EgressError::DuplicateDocument`）而不是被计数两次——
   重复身份会让「两桶之和 == 输入」这条判据照样成立，而清单本身是错的。**计数正确 ≠ 清单正确**。

9. **空集被清点并报 `0`**，MUST NOT 短路成「跳过」，也不被当成错误。

10. **存疑取值选不放行那一边**。列上是 `INTEGER`，SQLite 不把取值限死在 0/1。只有 `0` 是「未被标记」，
    任何其它值按**被标记**读（`!= 0`）。外发是危险方向：一个读不出来的值 MUST NOT 被读成「可以外发」。

11. **判定输入从存储读出**，MUST NOT 由调用方给——调用方给不了、也改不了那份上传范围。
    `remote_egress_targets` 的域是**活动快照 ∩ 未撤销 ∩ 该 `kb_scope`**。

12. **正向半与反向半同批落地**。§281 的原话是「只走本地词法检索或人工」——不外发**不等于**不可检索。
    只钉住反向半（被扣留）的判据，会让「扣留」在实现上退化成删除：删掉一个文档当然也不外发，
    但那是另一个（错误的）实现。正向半用**真实 FTS5 存储**钉住，夹具走 `split_document` + `tokenize`
    真实路径，MUST NOT 用 `synthetic_chunk` 那种按尺寸手搓的块——后者的 `tokens` 是 `t0` 这种占位值，
    于是断言会在**夹具**上失败，而不是在被断言对象上失败（判据的输入必须等于它要断言的那件事）。

13. **registry `S10` 保持 `implemented`，不升 `tested`**。子句一的判定面与执行体**已建并有测试覆盖**，
    但链条的最后一环**未闭合**：`RemoteGateway::sync` 不消费这份清单（其入参仍由调用方给），且 `sync`
    在仓库内**零生产调用点**（只有测试调用）。⇒ **没有任何代码路径会真的执行「按这份清单外发」**。
    这与 ADR 0108 的 `S07`（四面已建其二、保持 `implemented`）是同一条口径。

## 后果

- 建成的承载物：`migrations/sqlite/0010-document-local-only.sql` + 版本号成套六处同步；
  `crates/nai-knowledge-core/src/egress.rs`（判定面）；`Store::remote_egress_targets`（执行体）。
- **老行一律读到 `0`。** 它的含义是「**从未被标记**」（升级时点的历史事实），**不是**「已核实这份文档
  可以外发」——后者是一个没有人做过的核对，MUST NOT 被写成前者的同义词。这一点写进迁移文件注释、
  写进 `version_ten_upgrade_...` 用例、也写进轮次报告。
- **具名登记、本轮不修的弱化**：
  1. `sync` 不消费这份清单 ⇒ 「外发一定经过这份清单」没有结构性保证，只有「存在这样一份清单」。
  2. 写入侧改不了老行（见上一点）。
  3. **TS 生成物的内容漂移本机无判据**：注入把 `packages/contracts-ts/src/generated/config/backup.ts`
     的 `db_schema_version` 改回 9，`node --test tests/contracts/schema.test.mjs` **30/30 全绿**——
     那份判据只断言「文件存在与清单对齐」，不比内容。能抓它的只有 CI 的 `cargo xtask schema --check`
     （`xtask/src/schema.rs` 的 `("ts", "packages/contracts-ts/src/generated")` + `differences`），
     本机缺 protoc 3.21.12 跑不了。与 W129 已登记的同名缺口是**同一处**，本轮再确认一次，不新增判据。
- **注入验证暴露的一处判据退化（本轮修掉）**：域里 `JOIN knowledge_active_snapshots` 这一句，
  **最初没有任何既有用例能走到它那一层**——既有两次发布的用例里文档修订都变了，旧快照的行被
  `s.revision=d.current_revision` 天然挡掉。这是 W127 记下的第二种成因「注入不变红 ≠ 注入坏了」，
  但方向不同：这里不是判据被更早的闸门短路，而是**判据根本没有覆盖那一层**。
  处置是**补能走到那一层的输入**（两次发布且修订未变 / 两个 `kb_scope` 各有活动快照），不是改判据。
  补上后两条注入各红一条**不同**的用例 ⇒ 两条域判据互不顶替。
