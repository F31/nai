# W125 报告：R08「存储维护有界」的口径与名册（快照/发布族覆盖）

日期：2026-10-16｜归属：R1「本仓可闭合」项 R08｜**生产代码仅可见性 + 一处拒绝拆名，无行为语义改动**

## 一、名册（现测）
按「具名拒绝（或具名丢弃计数）」普查 `crates/nai-storage`，**约束单次写入或单份快照/备份可占用资源**
的上限共 **19** 项：

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

**本轮前只有 2 项有载体**（决策载荷、遥测缓冲容量）⇒ 其余 17 项可被抬高或删除而**零测试变红**。
口径（收什么、为什么不收读路径预算与输入形状校验）见 ADR 0103。

## 二、判据（真实证据，`nai-storage` 57 → 63）
- `the_declared_storage_write_bounds_are_the_magnitudes_the_item_claims`：**名册幅度判据**，逐项断言常量
  等于声明幅度。它管「上限悄悄变大」；边界判据管「闸门还在不在、边界在哪一侧」——夹具从常量派生，
  ⇒ **抬高常量不会让边界判据变红**，只有这条会。
- `the_import_bounds_are_enforced_on_both_sides_of_the_declared_cap`：内容与分块数**双侧**
  （恰在界上必须过、越一字节/一块必须具名拒绝）。
- `the_published_snapshot_bounds_are_the_sum_over_the_selected_documents`：发布上限按**所选文档合计**
  判定 ⇒ 两份各自合法的文档合起来必须被拒（逐文档测试会判成通过）；计数与字节越界**具名可区分**。
- `a_backup_over_the_declared_ceiling_is_refused_by_the_size_gate_not_by_hash`：数据库超限 ⇒
  `resource_exhausted / snapshot file limit`；**同尺寸改字节** ⇒ `conflict / database hash/size mismatch`
  ⇒ 两道闸门可归因。
- `a_backup_artifact_over_the_declared_ceiling_is_refused_by_the_size_gate`：读回侧的文件上限。
- `an_artifact_at_the_declared_ceiling_is_accepted_and_one_byte_over_is_refused`：**写入**侧同值同界。
- 顺带修掉唯一一处**复制字面量**：`"x".repeat(16 * 1024 * 1024 + 1)` → 引用 `MAX_CONTENT_BYTES`
  （W77 的同一条规矩）。
- 本机：`cargo test -p nai-storage --lib` ⇒ **63 passed; 0 failed**（22.07s）。

## 三、注入验证（每条判据都要证明它会响）
三条**独立**注入同时打进去，看各自是否真的变红：

| 注入 | 改法 | 实测 |
|---|---|---|
| α 读回尺寸闸门 | `checked_file(…, MAX_BYTES, op)` → `u64::MAX` | ✅ 两条快照尺寸判据红（`resource_exhausted` 断言失败）；**并**溢出 `checked_file:28` 的 `maximum + 1`，连带 4 条无关 restore 用例——**注入的连带，不是判据缺陷** |
| β 发布拒绝重新合并 | 计数/字节两条并回 `snapshot chunk bound` | ✅ 红，`left "snapshot chunk bound" / right "snapshot chunk count bound"`（⇒ 断言确实钉在**字节**分支上） |
| γ 内容界改 `>=` | 端点从「含」变「不含」 | ✅ 红（恰在界上被拒 ⇒ 双侧构造有效） |

**名册幅度判据**与 **artifact 写入判据**在三条注入下**保持绿** ⇒ 与设计一致：幅度 pin 管漂移、
边界判据管行为，两者互不代劳。注入已干净回退（`git diff` 三处源码仅剩可见性与拆名改动）。

## 四、registry 判定（**维持 `implemented`**）
名册 19 项已覆盖 8、**剩余 11 项零覆盖**（逐项具名在 registry notes 里）⇒ 覆盖仍属部分，**不升 `tested`**
（第四次使用「守卫通过 ≠ 本项达标」）。两处**口径订正**已就地写入 registry：
① W124 把门槛写成 4 项常量、且未给口径；② publication/knowledge 两族对应 D03 原文的
「登记的不可变文件**数量/大小/路径受限**」，不是 W124 那种四项点名的替代品。

## 五、门控（全量 `cargo xtask check --profile full`）
- `GATE_EXIT`：**0**；`GATE_OK at attempt 1`（**一次过**）。`LNK1104` = **0**（对比 W124 的 10 次重试）。
- 步骤全过：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、
  `cargo test --workspace --locked --exclude nai-xtask`、`cargo test --locked -p nai-xtask --lib`、
  `node scripts/frontend-gate.mjs`，以及 **13** 个 node 套件（各 `# fail 0`）。
- 测试目标 **95** 个全 `0 failed`（77 个 `Running` 目标 + **18** 个 `Doc-tests`；日志里首个 `Running` 是
  gate runner 自身回显，不计），合计 **997 passed / 0 failed**。
  门控内实测 `nai-storage` **63 passed; 0 failed**（含 W125 六条新增）。
- 告警：全程只有增量编译锁文件「拒绝访问 (os error 5)」，跨多个 crate 共有、与本轮改动无关。
- 本 profile **未包含** `cargo xtask schema --check`（它需要 protoc 3.21.12）⇒ 该步骤**不作「已通过」声明**。
- ⚠️ 首跑在 `cargo fmt --all --check` 停住：两条 `let … = stage(…).unwrap_err();` 换行不合 rustfmt
  （门控是 fail-fast，故当时还未跑到 clippy/测试）。改法：按 rustfmt 建议换行，未动语义。

## 六、提交 / 推送
（待跑）

## 七、CI 观测
（待跑）
