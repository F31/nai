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

共跑 **5 次**（均带 `CARGO_BUILD_JOBS=1`；r1–r4 带 `RUST_TEST_THREADS=1`，r5 带 `RUST_TEST_THREADS=4`）：

| 跑次 | 结果 | 耗时 | 红的成因 |
|---|---|---|---|
| r1 | 红 | 3m18s | `LNK1104` 打不开 `target/debug/deps/pipeline-12fd291c622b7a2b.exe`（test "pipeline"） |
| r2 | 红 | 0m33s | `LNK1104` 打不开 `engine-3b76f48caecf3f08.exe`（test "engine"） |
| r3 | 红 | 0m25s | `LNK1104` 打不开 `nai_engine-ccd542a792799fb7.exe`（bin "nai-engine" test） |
| r4 | **绿** | **7m51s** | ——（`RUST_TEST_THREADS=1`） |
| r5 | **绿** | **5m23s** | ——（`RUST_TEST_THREADS=4`，CI 限并发后的复核；计数与 r4 **逐项一致**，见下） |

**四条替代判据**（判「宿主瞬态」而非「代码缺陷」）：

| # | 判据 | 本轮实测 |
|---|---|---|
| ① | 错误类同族且全在 bring-up／夹具 | 三次都是 `LNK1104`，**全在链接期**——`test result` 计数为 **0**，一条测试都没跑；日志里同时刷满 `error deleting lock file for incremental compilation session directory … 拒绝访问 (os error 5)` |
| ② | 失败集合每次漂移、不绑定用例 | 三次红的**目标各不相同**且都在 `apps/engine` 包内：`pipeline` → `engine` → `nai_engine` bin |
| ③ | 相关文件 `git status` 为空 | `git status --porcelain -- apps/ crates/nai-engine` ⇒ **空**（本轮未改 `apps/`） |
| ④ | 同一份代码限并发下逐目标能过 | 单独跑 `cargo test -p nai-engine --test pipeline` ⇒ **13/13 绿**（15.85s）；`--test engine` ⇒ **2/2 绿**（1.64s） |

补充核实：**排除磁盘满**（E 盘剩 **48.3 GB**、`target/debug/deps/` 可写）。
⇒ 判为**宿主瞬态**（文件锁被外部进程占用），**未改任何代码**。

处置：改用**有界重试预热**（`cargo test --workspace --locked --exclude nai-xtask --no-run`，
上限 10 次）⇒ **第 6 次成功**把 workspace 的测试目标链接齐（前 5 次各红在不同目标），
随后门控**一次即绿**。

⚠️ 这再次印证 W130 的结论：**限并发只是减轻，不是消除**——r1–r3 全在
`CARGO_BUILD_JOBS=1` 下红。故结论不落在「限并发后全绿」上。

### r4 计数（最终绿，7m51s）

| 项 | 本轮 | W130 对照 |
|---|---|---|
| Rust | **1058 passed / 0 failed** | 1045（W129 为 1037）⇒ **+13** == 本轮新增 4 判定面 + 8 执行体 + 1 迁移，逐项吻合 |
| `^     Running ` | **81**（80 目标 + 1 条 cargo 自回显的 xtask 包装行） | 80（79 + 1）⇒ +1 个目标（新增 `tests/egress.rs`） |
| `   Doc-tests` | **18** | 18 |
| `test result` 条数 | **98**（80 + 18） | 97（79 + 18） |
| Node | **13 套件 324 passed / 0 failed** | 324/0（持平） |
| 错误类（`LNK1104` / `error[` / `out of memory` / `FAILED` / `panicked`） | **0** | 0 |

前端门控 7 步全过（`packages/contracts-ts` → typecheck/build、`apps/desktop` → typecheck/build/test、
`<root>` → test:ui），末行 `[frontend] 全部步骤通过（工程基础，非渲染/人工 UI 验收）`。

门控在**全部改动定稿之后**重跑，故日志字节与提交字节对应。

## 六、提交与推送

- 一次提交 **`c010374`**（22 files changed, **+1319 / −13**）
- 暂存命令：`git add -A -- . ':!.commitmsg_*.txt' ':!target-check' ':!.gate-w131.sh'
  ':!.prewarm-w131.sh' ':!.sha256-w131.txt'`（临时脚本与 SHA256 基线文件被显式隔离）
- 推送：`62695e7..c010374`（SSH 443，`ssh.github.com:443` + `StrictHostKeyChecking=accept-new`）
- 完整 SHA：`c010374be308531d413e71e1ec5c8f855b6d3740`；`git rev-parse HEAD origin/main` 两者一致

**第二次提交**（CI 限并发修复，见 §7.3）：**`d3797cc`**（4 files / +184 −3，
`.github/workflows/foundation.yml` + ADR 0109 勘误段 + 报告 §七 + dev-plan），
push `c010374..d3797cc`，完整 SHA `d3797cc4b921a7e826a882e5489ee260ddfd4512`。

## 七、CI 观测（触发确认 1 次 + 完成态 ≥2 次）

### 7.1 第一次观测（提交 `c010374`）⇒ **两平台都 failure**

观测三件齐：`cb=$(date +%s%N)` + `Cache-Control: no-cache` + 回读完整 40 位 `head_sha`。

| 观测 | 查询参数 | 回读 `head_sha` | 结论 |
|---|---|---|---|
| 触发确认（1 次） | `runs?per_page=3&cb=…` | `c010374be308531d413e71e1ec5c8f855b6d3740` == 推送 SHA ✓ | run **38992263793**，`in_progress` |
| 完成态（有界循环，间隔 120s，上限 10 次） | `runs?per_page=3&head_sha=<40 位>&cb=…` | 同上 ✓ | 前 5 次 `in_progress`，**第 6 次 `completed / failure`**（09:47→09:57） |

两平台的失败**不在同一步骤**：

| 平台 | job id | 失败步骤 | 结论 |
|---|---|---|---|
| ubuntu-24.04 | 117277007758 | 步骤 9 `Check generated contracts without modifying consumers`（`cargo xtask schema --check`），exit **101** | 后续 14 步全部 skipped |
| windows-latest | 117277007629 | **步骤 9 `success`**，步骤 10 `Complete engineering foundation`（`cargo xtask check --profile full`）failure | 后续 skipped |

⚠️ 注意 windows 的步骤 9 **success** ⇒ 本轮版本号成套第六处（TS 生成物 `backup.ts` 的
`db_schema_version: 10`）**已被 CI 的生成器核对通过**，与 `backup.schema.json` 的 const 一致。

### 7.2 定位：CI 日志与 annotation

`GET /actions/jobs/{id}/logs` 匿名返回 **403**（`Must have admin rights to Repository.`）⇒ 读不到 CI 日志。
改走 W130 建的诊断通道：`GET /repos/…/check-runs/{id}/annotations`（**匿名可读**）⇒ windows **12** 条 /
ubuntu **2** 条。windows 的失败行：

```
test tests::storage_bounds::the_managed_artifact_count_bound_counts_distinct_paths ... FAILED
thread '...' panicked at crates\nai-storage\src\tests\storage_bounds.rs:398:27:
called `Result::unwrap()` on an `Err` value:
  StorageError { code: DeadlineExceeded, reason: "operation stopped", position: None }
test tests::storage_bounds::the_revocation_batch_bound_counts_distinct_document_revisions
  has been running for over 60 seconds
```

⇒ `Operation` 的 **5s 期限**被超过（W84 起登记的同一族签名）。

### 7.3 本机复现与对照实验（**判为既有基线，与本轮改动无关**）

本机门控此前是**手工**带 `RUST_TEST_THREADS=1` 跑的，而 CI 用**默认并行度** ⇒ 两者不一致，
这正是「本机门控绿而 CI 红」的机制性原因。用 **CI 同款并行度**复现：

| 实验 | 结果 |
|---|---|
| 带本轮新增 8 条，默认并行度 ×3 | 3 次里 **2 次红**（81 条中红 3 / 0 / 1 条），红的用例**每次都不同**：`approval_workflow::list_is_bounded…`、`decision_log::critical_write…`、`a_backup_artifact_over_the_declared_ceiling…`、`approval_workflow::approved_candidates_cannot_be_rejected` |
| **摘掉**本轮新增 8 条，默认并行度 ×3 | **3 次全红**（71 条中红 2 条/次）：`decision_log::failed_flush_rolls_back…`、`configuration_uses_same_schema…`、`approval_workflow::stored_display_text…`、`an_artifact_at_the_declared_ceiling…` |
| 带本轮新增 8 条，`RUST_TEST_THREADS=4` ×3 | **3 次全绿**（81/81，78.06s / 77.07s / 79.17s，较默认并行度只增约 10s） |

⇒ **对照实验是决定性的**：摘掉本轮新增用例后并行度下**仍然 3 次全红**，而带上时还有 1 次全绿
⇒ 该不稳定**与本轮改动无关**，是既有基线（且不是「最后一根稻草」）。红的全部是**既有**的期限敏感
用例，本轮新增的 8 条 egress 用例**一次都没红过**。

**处置**：CI 与本机门控统一限并发到 4（`.github/workflows/foundation.yml` 门控步加
`env: RUST_TEST_THREADS: "4"`）。这是**环境适配，不是放宽判据**——没有改任何期限常量、
没有改任何断言。改后本机 `workflow.test.mjs` **14/0**、`step-config.test.mjs` **16/0** 仍绿。

⚠️ **具名登记、本轮不修的根因**：仓库里有一批**期限敏感**的断言（依赖 `Operation` 的 5s 期限），
在 CPU 争抢下会**误报**为「操作超时」。真正的修法是让这些断言不依赖墙钟/负载（W84 的同族问题），
超出本轮范围。

### 7.4 ubuntu 的 `schema --check`（**无法本机复现，具名登记**）

ubuntu 步骤 9 报 exit **101**，而 windows 同一步 **success**；W130 那次 CI（run 38982577116）
两平台的步骤 9 都是 success ⇒ 变化发生在这两轮之间。

本机**无 protoc**（`which protoc` 为空），而 `xtask/src/schema.rs::generate` 的第一件事就是
校验 `protoc --version` 必须逐字等于 `libprotoc 3.21.12` ⇒ **本机跑不了该入口，无法复现**。
本轮与生成相关的改动只有两处（`backup.schema.json` 的 const 9→10、`backup.ts` 的 9→10），
而这两处已被 **windows 的 success 验证一致**；`crates/nai-contracts/src/generated/` 下
**没有** config 生成物（只有 `catalog.rs`/`control_http.rs`/`nai.v1.rs`/`remote_knowledge.rs`），
不存在第七处未同步的承载物。

⇒ 判定：**无法定性**，随下一轮 CI 重跑观察（若为环境/protoc 安装漂移，重跑即绿）。

### 7.5 第二次观测（限并发后的提交 `d3797cc`）⇒ ubuntu **success** / windows **cancelled**

| 观测 | 结果 |
|---|---|
| 触发确认（1 次） | run **38995278526**，`in_progress`，`head_sha` == 推送 SHA ✓ |
| 完成态（有界循环：第一轮 10 次 × 120s **全部 `in_progress`**；第二轮第 1 次） | **`completed / cancelled`**（10:32 → 10:53） |

逐平台：

| 平台 | 结论 | 耗时 | 关键步骤 |
|---|---|---|---|
| ubuntu-24.04 | **success** | **457s**（7m37s） | 步骤 9 `schema --check` **success**；步骤 10 门控 **success** |
| windows-latest | **cancelled** | **1210s**（20m10s） | 步骤 9 **success**；步骤 10 门控 **success**；步骤 12–13 **success** |

⇒ 两点结论：

1. **ubuntu 上一轮的 exit 101 是环境瞬态，重跑即绿**；本轮与生成相关的两处改动
   （`backup.schema.json` 的 const 9→10、`backup.ts` 的 9→10）在**两平台**都已被 `schema --check` 核对一致。
2. **windows 的 `cancelled` 不是失败**：门控步本身 **success**（⇒ 限并发到 4 确实消除了期限超时），
   但整个 job 在 1210s 被砍——workflow 的 `timeout-minutes: 20` 不够用了。
   W130 实测 windows 全跑完 15m43s（另一次 15m55s，已逼近上限），限并发后必然超出。

**处置**：`timeout-minutes: 20` → **35**（提交 **`ed28c5f`**，2 files / +9 −1）。
⚠️ 这是**预算调整，不是放宽判据**：没有改任何期限常量、断言或步骤集合；
`workflow.test.mjs` **14/0**、`step-config.test.mjs` **16/0** 仍绿。

### 7.6 第三次观测（超时放宽后的提交 `ed28c5f`）⇒ **两平台 success**

| 观测 | 结果 |
|---|---|
| 触发确认（1 次） | run **38996838549**，`in_progress`，`head_sha` == 推送 SHA ✓ |
| 完成态（有界循环，间隔 150s，上限 14 次） | 前 8 次 `in_progress`，**第 9 次 `completed / success`**（10:56 → 11:17） |
| 完成态第 2 次确认 | 同上：`completed / success`，`head_sha` 回读一致 ✓ |

| 平台 | 结论 | 耗时 | 步骤 9 `schema --check` | 步骤 10 门控 | 非 success 且非 skipped 的步骤 |
|---|---|---|---|---|---|
| ubuntu-24.04 | **success** | 521s（8m41s） | success | success | **无** |
| windows-latest | **success** | 1140s（19m00s） | success | success | **无** |

⇒ 本轮闭环。三次观测串起来看：

1. 第一次（`c010374`）两平台 **failure**：windows 是既有期限超时（对照实验判定与本轮改动无关），
   ubuntu 是 `schema --check` 的环境瞬态。
2. 第二次（`d3797cc`，限并发 4）：ubuntu **success**；windows 门控步 **success** 但整个 job 在
   1210s 被 `timeout-minutes: 20` **cancelled**（跑不完，不是失败）。
3. 第三次（`ed28c5f`，超时 35）：**两平台 success**，windows 实际只需 **1140s（19m00s）**
   ——未超原 20 分钟上限，说明第二次的 1210s 里含有机器波动；超时放宽到 35 是为这种波动留余量。

⚠️ **具名登记**：windows CI 总时长已达 **19 分钟**，即便放宽到 35 也只剩一半余量；
`timeout-minutes` 与「限并发换稳定」这两件事要一起看，后续若再加 suite 需重新评估。

API 用量：第一次 **11** 次（触发 1 + 完成态 6 + jobs 1 + check-runs 1 + annotations 2 +
对照用的 W130 jobs 1）；第二次 **13** 次（触发 1 + 完成态 11 + jobs 1）；第三次 **11** 次
（触发 1 + 完成态 9 + jobs 1 + 完成态第 2 次确认 1）。合计 **35** 次。
