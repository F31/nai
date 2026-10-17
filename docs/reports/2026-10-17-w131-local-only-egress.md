# W131：S10 子句一「`local_only` 不外发」——判定面、执行体与承载物

- 日期：2026-10-17｜归属轮次：W131｜ADR：0109
- 规格差量：`knowledge-pipeline` ADDED 三条 Requirement
- 结论：registry `S10` **保持 `implemented`，不升 `tested`**（链条最后一环未闭合，见 §四）

## 一、缺口（现测，MUST NOT 由「同族」推断）

`tests/registry.yaml` 的 `S10` notes 此前逐字写着「**`local_only`/凭证边界已实现**；长期画像未建」。
开工前现测：字面 `local_only` 在 `crates/`、`apps/`、`xtask/`、`scripts/`、`tests/` 里**零命中**——
唯一出现它的地方就是 registry 这一行自己。**这是一条可判事实的不实陈述：它没有实现体。**

基线 `docs/直播数字人 + 智能体系统技术方案 V3.11.md` 里 `local_only` 出现 4 次，口径一致：

| 位置 | 原文要点 |
|---|---|
| 行 **89**（INV-17） | 「数据外发范围……受 `local_only` 约束」 |
| §**13.2** | 「`local_only` 数据不上传」 |
| §**281** | 「标记 `local_only` 的文档**只走本地词法检索或人工**」 |
| §**563** | 「上传范围」受 `local_only` 约束 |

`S10` 的验收口径是三条子句，逐条现测：

| 子句 | 现测 | 证据 |
|---|---|---|
| ① `local_only` 不外发 | **零落点** | 全仓 `grep -rn local_only` 只命中 registry 自己 |
| ② 画像默认关闭 | **已建且真执行** | `MemoryPolicy` 六项在 `admit_write` 里执行（`evaluate/memory.rs` 行 189–222）：`allowed_viewer_fields` / `forbidden_sensitive` / `max_retention_ms` / `max_content_chars` 都有执行点，不是只声明 |
| ③ 撤销与删除按期限清除 | **已建** | `ForgetTrigger::ALL` 含 `Ttl`；`revoke_knowledge` + `purge_memory_body` 已接线 |

⇒ 本轮范围**只有子句一**。②③ 不动。

一并现测的相邻事实（决定接线位置）：
- `documents` 表原有列：`workspace_id, document_id, source, content_hash, product_scope, status,
  acl_ref, original_artifact_ref` —— **无 `local_only` 列**（`migrations/sqlite/0001-foundation.sql`）。
- `RemoteDocument` 在仓库里**只有 `sync` 的入参**；`sync` 本身**零生产调用点**（只有测试调用），
  与 d09 报告「真实远端 Blocked」一致。
- `nai-runtime` **完全不含**知识装配 ⇒ 接线落点在 `nai-storage`（清单）+ `nai-knowledge-core`（判定）。
- 存储侧已有先例 `remote_delete_targets`（为组合根产出**面向远端的清单**：`kb_scope` 限定 +
  `LIMIT 16` 与 `RemoteDelete` 的 `maxItems` 对齐）⇒ 本轮同形。
- `remote.rs` 里的 `16` **硬编码两处**（`sync` 行 642 / `delete` 行 671）——同形不同义。
  ⇒ 判定面**不**再定义一次上限（否则是第三处定义）。

## 二、改动与判据（真实证据）

### 2.1 承载物

- 新增 `migrations/sqlite/0010-document-local-only.sql`：
  `ALTER TABLE documents ADD COLUMN local_only INTEGER NOT NULL DEFAULT 0;`
- 版本号成套**六处**同步（`migration::VERSION` 9→10）：

| # | 位置 | 改动 |
|---|---|---|
| 1 | `crates/nai-storage/src/migration.rs` | `VERSION: i64 = 10`、`SQL: [&str; 10]` + `include_str!` |
| 2 | `schemas/config/backup.schema.json` | `db_schema_version.const` 9 → 10 |
| 3 | `crates/nai-storage/src/tests/knowledge_publication.rs` | 契约样本断言 9 → 10 |
| 4 | `tests/contracts/schema.test.mjs` | 备份样本 9 → 10，**含「上一版 MUST 被拒」+1**（8 → 9） |
| 5 | `packages/contracts-ts/src/generated/config/backup.ts` | TS 生成物 9 → 10 |
| 6 | 迁移文件本身 | 新增 `0010-document-local-only.sql` |

- `ImportedDocument.local_only`：**必填、无默认** ⇒ 每一处构造都必须表态。

### 2.2 判定面（`crates/nai-knowledge-core/src/egress.rs`，新增）

`EgressCandidate { document_id, revision, local_only }`、`EgressWithholding::LocalOnly`（`label()`
= `"local_only"`）、`EgressPlan { releasable, withheld }`（字段私有、无 `Default`）、`plan_egress`。

四条判据（`crates/nai-knowledge-core/tests/egress.rs`）：
1. 具名扣留 + 两桶闭世界（元素**恰好**是输入）+ 被标记者 MUST NOT 出现在可外发桶
2. 空集报 `0`（不短路成跳过、也不是错误）
3. 判定单元是 `(document_id, revision)`（同名不同修订是两份）
4. 重复身份**被拒**（不是被计数两次）

### 2.3 执行体（`Store::remote_egress_targets`，新增）

```
SELECT d.document_id, d.current_revision, d.local_only
  FROM documents d
  JOIN knowledge_snapshot_documents s ON s.workspace_id=d.workspace_id
    AND s.document_id=d.document_id AND s.revision=d.current_revision
  JOIN knowledge_active_snapshots a ON a.workspace_id=s.workspace_id
    AND a.snapshot_id=s.snapshot_id
 WHERE d.workspace_id=?1 AND a.kb_scope=?2 AND d.status!='revoked'
   AND d.current_revision IS NOT NULL
 ORDER BY d.document_id
```

域 = **活动快照 ∩ 未撤销 ∩ 该 `kb_scope`**。标记从 `documents.local_only` **读出**，MUST NOT 由调用方给。
`local_only: row.get::<_, i64>(2)? != 0` —— 只有 `0` 是「未被标记」，任何其它值按**被标记**读。

八条判据（`crates/nai-storage/src/tests/egress_targets.rs`）：
1. 被标记者进具名扣留桶 + 两桶闭世界 + **仍被本地 FTS5 检索返回**（正向半）
2. 重新导入改标记 ⇒ 清单跟着走（标记由存储读出）
3. 已撤销 ⇒ 不在任何一个桶（撤销与扣留是两个不同的原因）
4. 列上非 0 非 1 的存疑取值 ⇒ 按**被标记**读
5. 还没有活动快照 ⇒ 域是空集，报 `0`（不是错误、不是跳过）
6. 发布之后才导入的文档 ⇒ 在域外
7. 域是**活动**快照，不是任何曾发布过的快照（修订未变时同一文档只在活动快照里算一次）
8. 外发域按 `kb_scope` 界定，另一个域的文档 MUST NOT 混进来

外加 1 条迁移用例（`version_ten_upgrade_keeps_documents_and_reads_the_new_mark_as_unmarked`）：
在**已有行**的库上升 0010，断言老行读到 `0`、文档与修订都还在。

### 2.4 刻意不做（三条）

1. **不再回答「上传是否被允许」** —— 那是 `sync` 的 `allow_document_upload`。同一个开关答两次会漂移
   （W88 / PD-31），所以 `plan_egress` **没有** policy 入参。
2. **不引入新的条数上限** —— 可外发集合的规模由 `MAX_SNAPSHOT_DOCUMENTS` 界定；再造一个数是同一件事的
   第二处定义，而且那个闸门**不可能开火**（域里永远到不了界）。永远开不了火的守卫是装饰，不是判据。
3. **不动 `sync` 的入参** —— 见 §四。

### 2.5 判据绿（fmt-clean 之后）

| 目标 | 结果 |
|---|---|
| `cargo fmt --all --check` | **红 1 处 → 就地修掉 → 绿**（新加用例里的 `execute(...)` 需折行） |
| `cargo test -p nai-knowledge-core` | **55 passed / 0 failed**（含新增 `egress` 目标 4 条） |
| `cargo test -p nai-storage --lib` | **81 passed / 0 failed**（新增 8 条外发 + 1 条迁移） |
| `node --test tests/contracts/schema.test.mjs` | **30 passed / 0 failed** |
| `node --test tests/registry.test.mjs` | **56 passed / 0 failed** |
| `cargo clippy -p nai-knowledge-core -p nai-storage --all-targets --locked -- -D warnings` | exit 0 |

⚠️ fmt 那条不是"顺手"：**注入验证的基线 MUST 是 fmt-clean 的最终字节**（W129 的坑）。
取基线前先跑 fmt，发现红就先修——否则 `sha256sum -c` 的「逐字节还原」核对的是一个过渡态。

## 三、注入验证（每条判据都要证明它会响）

**基线**：`cargo fmt --all --check` 干净之后取 `sha256sum`。**13 条注入真跑**，逐条改文件 → 真跑 → 还原。

| # | 注入（改的是**实现**，不是测试） | 结果 |
|---|---|---|
| 1 | `if candidate.local_only` → `if false`（扣留判据不再开火） | **红 2 条**，断言落在行 46「未被标记的文档按原序可外发」 |
| 2 | `withheld.push(...)` → `drop(candidate)`（**静默丢弃**） | **红 2 条**，断言落在行 41「每个候选必须恰好落进一个桶」 |
| 3 | `return Err(DuplicateDocument)` → 提前返回 Ok（重复身份静默去重） | **红 1 条** |
| 4 | `seen.insert((id, revision))` → `seen.insert(id)`（判定单元降级为名字） | **红 1 条** |
| 5 | `plan_egress` 开头加「空集返回 Err」（空集短路成错误） | **红 1 条** |
| 6 | SQL 里 `d.local_only` → 常量 `0`（标记不从存储读） | **红 3 条** |
| 7 | 去掉 `AND d.status!='revoked'` | **红 1 条**（`a_revoked_document_is_in_neither_bucket`） |
| 8 | `JOIN knowledge_active_snapshots` → `JOIN knowledge_snapshots`（域扩成「任何曾发布过的快照」） | **红 1 条**（`the_release_domain_is_the_active_snapshot_...`，`duplicate_document`） |
| 9 | `a.kb_scope=?2` → `(a.kb_scope=?2 OR 1=1)`（作用域过滤失效） | **红 1 条**（`a_document_outside_the_queried_scope_...`） |
| 10 | `!= 0` → `== 1`（存疑取值按「未标记」读） | **红 1 条** |
| 11 | 撤掉迁移 0010（`VERSION` 回 9、数组回 9） | **红 1 条**（`no such column: local_only`） |
| 12 | `packages/contracts-ts/src/generated/config/backup.ts` 的 `db_schema_version` 10 → 9 | **本机不红**（30/30 全绿）——见下 |
| 13 | 导入 INSERT 的 `local_only` 参数 → 常量 `0`（**写入侧**不写标记） | **红 2 条** |

**注入 1 与 2 红的断言层不同**（行 46 vs 行 41）⇒ 「具名扣留」与「闭世界」是两条**独立**的判据，
不是同一件事写了两遍。**注入 8 与 9 红的用例不同** ⇒ 两条域判据互不顶替。

**还原**：5 个被注入文件全部 `sha256sum -c` 通过（`egress_targets.rs` 的差是新增两条用例，合法）。

### 3.1 注入 12 为什么本机不红（**具名登记，本轮不修**）

`tests/contracts/schema.test.mjs` 里那份 TS 生成物判据只断言**「文件存在与清单对齐」**
（`generatorErrors(TS_GENERATED_SOURCES, validated, generated)`），**不比内容**。
⇒ 生成物的 `db_schema_version` 与 `backup.schema.json` 的 const 漂移，本机抓不到。

能抓它的是 CI 的 `cargo xtask schema --check`（`xtask/src/schema.rs` 的
`("ts", "packages/contracts-ts/src/generated")` + `differences`），本机因缺 protoc 3.21.12 跑不了。
这与 W129 已登记的同名缺口是**同一处**，本轮再确认一次，**不新增判据**。

### 3.2 一处判据退化（本轮**修掉**）

跑到注入 8 时发现：域里 `JOIN knowledge_active_snapshots` 这一句**最初没有任何既有用例能走到它那一层**——
既有两次发布的用例里文档修订都变了，旧快照的行被 `s.revision=d.current_revision` 天然挡掉，
于是那条 JOIN 当次**无覆盖**，怎么改都不会红。

⚠️ 这是 W127 记下的第二种成因「注入不变红 ≠ 注入坏了」，但**方向不同**：这里不是判据被更早的闸门短路，
而是**判据根本没有覆盖那一层**。处置是**补能走到那一层的输入**，不是改判据：
- `the_release_domain_is_the_active_snapshot_not_any_snapshot_ever_published`（两次发布、修订未变）
- `a_document_outside_the_queried_scope_is_outside_the_release_domain`（两个 `kb_scope` 各有活动快照）

### 3.3 由**构造**保证、因此不需注入的事实（如实标明，MUST NOT 记作「已注入验证」）

- `ImportedDocument.local_only` **必填且无默认**：不给就编译不过（Rust 类型层面），
  没有任何注入能令其「变红」。它不是缺陷，但 MUST NOT 记作已注入验证。
- `EgressPlan` 字段私有且无 `Default`：只能由 `plan_egress` 造出，同属构造性保证。

## 四、registry 判定（**保持 `implemented`，不升 `tested`**）

按本仓四环交付模型：

| 环 | 状态 | 说明 |
|---|---|---|
| ① 判定面 | **已建** | `nai-knowledge-core::egress::plan_egress`，4 条测试 + 5 条注入全红 |
| ② 执行体 | **已建** | `Store::remote_egress_targets`，8 条测试 + 5 条注入（含写入侧）全红 |
| ③ 调用方 | **未闭合** | `RemoteGateway::sync` **不消费**这份清单（其入参仍由调用方给） |
| ④ 真实产生者 | **无** | `sync` 在仓库内**零生产调用点**（只有测试调用），真实远端 Blocked |

⇒ **没有任何代码路径会真的执行「按这份清单外发」**。这与 ADR 0108 的 `S07`
（四面已建其二、保持 `implemented`）是同一条口径。

`S10` notes 已就地 1:1 订正（原句「`local_only`/凭证边界已实现」是不实陈述），
`status` **保持 `implemented`**。另两条子句本轮现测**已建且真执行**；**长期画像未建**（原有缺口，未变）。

## 五、门控（`cargo xtask check --profile full`）

_（回填）_

## 六、提交与推送

_（回填）_

## 七、CI 观测（触发确认 1 次 + 完成态 ≥2 次）

_（回填）_
