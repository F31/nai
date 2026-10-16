# W129 报告：存储上限名册的行为覆盖（R08 收口）

- 轮次：W129｜日期：2026-10-16｜归属：`tests/registry.yaml` **R08**（存储维护有界）
- 关联：[ADR 0107](../adr/0107-storage-bound-behavior-coverage.md)、ADR 0103（W125 名册）、
  ADR 0100（`tested` 门槛）、`openspec/changes/2026-10-16-w129-storage-bound-roster-coverage/`、
  审计 `docs/reports/2026-10-16-state-of-implementation-audit.md`（缺口③：仓库内可闭环）

## 一、缺口（现测）

**缺口逐字来自 registry，不来自散文。** R08 的 W125 段原话：
「**仍不升 `tested`**：名册 19 项**剩余 11 项零覆盖**」，并**具名**列出这 11 项。
W126 审计的推进条件原文：「剩 11 项是"照方抓药"，且**全部覆盖完毕后 R08 才谈升 `tested`**」。

**「零覆盖」的实测判据**：对 11 条拒绝名逐条 `grep -rn` 全仓，命中数**均为 1**
（只有定义处）⇒ 这些闸门可以被删掉、改成恒真、或把边界挪一格，而**没有任何一条测试会红**。
W125 的幅度判据管的是「上限是不是它自称的那个数」（常量 vs 声明值），它**不**管
「闸门还在不在、边界在哪一侧」（行为）。

**本轮前 `nai-storage` 全量：63 passed / 0 failed；R08 名册 19 项覆盖 8 项。**

## 二、改动与判据（真实证据）

### 2.1 生产代码（12 个具名常量 + 拆一处 + 两处读窗口派生）

| 文件 | 改动 | 性质 |
|---|---|---|
| `store.rs` | 新增 `MAX_CONFIGURATION_BYTES` / `MAX_FACT_BYTES` / `MAX_CANDIDATE_BYTES` / `MAX_CANDIDATE_DEPENDENCIES` / `MAX_PENDING_CANDIDATES` | 常量提取，三处字面量改引常量 |
| `store.rs` | 候选路径的合并 `if` 拆成 `candidate dependency limit` / `candidate payload limit` | **唯一的语义变化**（`ErrorCode` 不变，仅 `reason` 更精确） |
| `knowledge.rs` | 新增 `MAX_EVIDENCE_REFS` / `MAX_REVOCATION_REVISIONS`；撤销读窗口 `LIMIT 4097` → `LIMIT 上限 + 1` | 常量提取 + 读窗口派生 |
| `publication.rs` | 新增 `MAX_SNAPSHOT_DOCUMENTS`；`len() > 256` 改引常量 | 常量提取 |
| `snapshot.rs` | 新增 `MAX_SNAPSHOT_ARTIFACTS`；artifact 读窗口 `LIMIT 513` → `LIMIT 上限 + 1` | 常量提取 + 读窗口派生 |
| `budget.rs` | 新增三条常量；`encode` / `decode` 改 `pub(crate)`（判据直接问这两条闸门）；⑪ 的三条件 `if` 拆成多行 | 常量提取 + 可见性 |

**无取值改动、无 `ErrorCode` 改动。** 三条同名数值 `65536`（配置文本 / 事实编码 / 候选编码）是
**不同的对象** ⇒ 三个常量（共用一个会让「改其中一条」不知不觉改另外两条）。

### 2.2 判据（`nai-storage` 63 → **72**）

| 计数 | 数值 |
|---|---|
| `crates/nai-storage/src/tests/storage_bounds.rs`（**新文件**） | 0 → **8** |
| `tests.rs` 的名册条目 | 8 → **20** 个常量（**19 项**；⑥ = 两条件一条目） |
| `tests.rs` 新增「名册闭世界」判据 | 0 → **1** |
| `nai-storage` 合计 | 63 → **72** |

8 条行为判据各自钉住什么（每条上限一对夹具：**恰在界上必须过 / 越界必须具名拒绝**）：

- `the_configuration_and_fact_encoding_bounds_are_refused_by_name`（④⑤）：`plugin_catalog` 是 schema 里
  唯一的自由字符串 ⇒ 把 JSON 顶到**恰好** `cap`；⑤ 用「单调滚动定界」找最大的 `pad` 使编码恰好 `cap`，
  并**断言** `at_cap == cap` 与 `over_cap > cap`。**⑤ 的夹具是 `publish_fact` 三处改写
  （`revision`/`publisher_ref`/`permission_revision`）的不动点**，且用例**断言**「落库字节 == 夹具测量」
  ——镜像一旦与生产代码分叉，用例会红。（本轮修正过一次：镜像把 `workspace.permission_revision`
  读成 `String`，实测是 `INTEGER` ⇒ 夹具红，随即按生产路径订正。）
- `the_candidate_bounds_are_separately_attributable`（⑥两支 + ⑧ + 正对照）：依赖条数、候选编码、
  证据引用三条闸门各自举证；正对照「依赖恰 32 + 证据恰 32」必须被接纳。
- `the_pending_candidate_capacity_is_a_count_of_unapproved_work`（⑦）：第 200 条自己过得去、第 201 条才被拒。
- `the_snapshot_document_count_bound_is_attributable_to_the_count`（②）：空选集与越界共用一条拒绝名的
  **两个分支各自举证**；越界那侧故意混进一个**不存在**的文档，好让拒绝**只能**来自条数
  （有探针先确认「不存在」走的是另一条具名拒绝）。
- `the_managed_artifact_count_bound_counts_distinct_paths`（③）：512 条不同内容路径可导出，第 513 条被拒。
- `the_ledger_write_bound_is_defensive_and_its_gate_has_two_sides`（⑨）：接缝两侧 + 一条**会过期**的
  可达性断言 + `MAX_STORED_LEDGER_HEX == MAX_LEDGER_ENTRY_BYTES * 2`。
- `the_stored_ledger_gates_are_separate_and_attributable`（⑩⑪）：⑪ 的三条件（奇长度 / 超长 / 非十六进制）
  逐个举证；**恰在界上那一侧先断言 `len() < text_cap`**，证明它没被 ⑩ 短路（W127 的「恰能走到它那一层」）。
- `the_revocation_batch_bound_counts_distinct_document_revisions`（①）：**真实路径**凑界
  ——16 轮 × 256 份文档 = 4096 对不重复 `(document, revision)`；界上放行、第 4097 对被拒且不落任何撤销记录。

**新增的「名册闭世界」判据**（`every_declared_crate_constant_is_classified_by_the_roster`）三层：
① 文件层（`lib.rs` 的每个 `mod` 都有被扫源文件）；② 元素层（每个**顶格**常量**恰好**归入三类之一，
三分是划分）；③ 反方向（三张清单上的名字都真的扫得到）。谓词按「顶格」而**不是**按可见性筛
——按可见性筛是**弱类别**，私有的 `MAX_DIMENSION` 会从后门溜走（首跑时这条守卫**抓到了它自己的作者**：
`tests::STORAGE_BOUND_ROSTER` 未被归类 ⇒ 守卫确实活着）。

## 三、注入验证（每条判据都要证明它会响）

编辑工具注入 + 真跑 + `sha256sum -c` 逐字节还原。**注入前基线**（`.w129-base.sha256`，7 文件）：

```
crates/nai-storage/src/store.rs                          89cf208b01c61fccd68d1154e5a1d012e88ee481d90410792c07de4974b71913
crates/nai-storage/src/knowledge.rs                      3894599f8b70740faf95fa3d3c157da190538bb5593a9b29e1d8196d7841ce21
crates/nai-storage/src/publication.rs                    071fc7ab479696db93d7e210c6c05275cb6c2937fb286bf7c9a0c326fa8da1e2
crates/nai-storage/src/snapshot.rs                       ce93d96c96e74d5a56fe3facdad64d00d3060834b769abe1b6ff71a525de8e10
crates/nai-storage/src/budget.rs                         1257b70342f5fe8aefb4a460d6a142fc91ce9131e338648cc0cfe6f2691a6a99
crates/nai-storage/src/tests.rs                          9dfc87980fba135e6e72e84f9de5696848d34a4dd4006c94b41e250beceec357
crates/nai-storage/src/tests/storage_bounds.rs           948f7985943173918fda8b6ad77197b655918047528922cb63b69731477431da
```

**8 组 / 17 条**注入，逐条确认「对应判据变红且**可归因**」：

| 组 | 注入 | 归属判据 | 红的位置 / 证据（原文） |
|---|---|---|---|
| R1 | ④ `>` → `>=` | test1 | `save_config(...).unwrap()` → `configuration too large` |
| R1 | ⑦ `>=` → `>` | test3 | `.unwrap_err()` on `Ok`（越界那条被接纳） |
| R1 | ⑧ `>` → `>=` | test2 | 正对照被拒 `candidate evidence bound` |
| R1 | ③ 窗口去 `+1` | test5 | `.unwrap_err()` on `Ok`（513 条不再触发） |
| R1 | ⑨ `>` → `>=` | test6 | `encode(&sized(pad)).is_ok()` 断言失败 |
| R1 | ⑩ `>` → `>=` | test7 | `.unwrap()` on `oversized stored ledger` |
| R2 | ⑤ `>` → `>=` | test1 | `publish_fact_filled(pad).unwrap()` → `fact too large` |
| R2 | ② 计数 `>` → `>=` | test4 | 界上 256 份文档被拒 `invalid snapshot documents` |
| R3 | ⑥ 依赖分支整体删除 | test2 | 拒绝换成后续闸门 `fact absent or expired` |
| R3 | ② 空选集分支删除 | test4 | 空选集掉到下游 `snapshot chunk count bound` |
| R4 | ⑥ 载荷分支删除 | test2 | `.unwrap_err()` on `Ok`（越界载荷被接纳） |
| R4 | ⑪ 长度条件删除 | test7 | 越界 hex 落到 `invalid stored ledger protobuf` |
| R5 | ⑥ 重合并为单 `if` 单名 | test2 | 载荷夹具拿到 `candidate dependency limit` |
| R6 | ① 窗口去 `+1` | test8 | 第 4097 对静默放行 `Ok(256)` |
| R7 | ① `>` → `>=` | test8 | 界上 4096 批次被拒 `revocation revision bound` |
| R8 | 名册幅值改 65535 | test9 | `store::MAX_FACT_BYTES` left=65536 right=65535 |
| R8 | 新增未归类顶格常量 | test10 | 闭世界报 `["store::MAX_PROBE_BYTES"]` |

- **对照（分工成立）**：R1 六条只动**比较符/窗口**、不动**常量值** ⇒ 名册幅度判据 **留绿**
  ⇒ 证明「幅度钉管上限悄悄变大、行为钉管闸门与边界」这条分工是真的，两条判据不互相顶替。
- **R5 是本轮方法论的关键一条**：把 ⑥ 重新合并成一个 `if` 后，**载荷**夹具拿到的是
  **依赖**拒绝名 —— 正是 W125 拆名的理由，且这是**只有**「重合并」能暴露的一条。
- 每次注入后逐字节还原，**末次 `sha256sum -c` 7/7 `OK`**；随后全量 `cargo test -p nai-storage`
  **72 passed / 0 failed**（无关判据未受损伤）。

**如实披露**：R1 的 ⑥ 未注入（⑥ 的三种注入放在 R3/R4/R5，各自单独跑）；R6/R7 是 ① 的两侧，
各自单独跑（合并注入会互相掩盖）。上述 8 组互不掩盖、逐一归因。

## 四、registry 判定（**升 `tested`**）

判据来自 ADR 0100 §6 与 `admission-registry` spec §Honest backfill：「`tested` 仅可用于
**已有归档实测证据**的项目」「未完成或**外部阻塞**的目标 MUST 保留 implemented/blocked」。
逐项现测：

1. **名册 19 项全部有载体**：19 项 = 20 个常量（⑥ 两条件一条目）；每项同时有**幅度钉**（常量 == 声明值）
   与**行为钉**（界上过 / 越界具名拒）。
2. **闭世界成立**：`nai-storage` 的每个顶格常量都归入「名册 / 按口径不收（形状校验）/ 不是上限」三类之一，
   且三分互不相交、三张清单都有对应源。
3. **验收原文对照**（D03 报告）：「原文/许可/素材或模型清单经登记的不可变文件纳入备份；
   **数量/大小/路径受限**」——「数量/大小」由名册承载，「路径」由 D03 测试 10 的穿越/symlink 拒绝 +
   ③ 的路径计数承载；WAL/事务/备份由 D03 的 12 项存储测试承载。
4. **无剩余缺口**（逐项现测）：审计对本项只列了「名册 11 项零覆盖」一条，现已清空。

**升 `tested`**（`implemented → tested`）。语义上不改 met/unmet 归类（`MET_STATUSES` 含二者）。

**如实披露**：⑨ 的上限**今天从被认可的路径走不到**（`nai-budget-core` 把字符串限在 256 字符、
`units` 限在 32 键 × 64 字符 ⇒ 最大合法条目仅几 KB）；本条已具名登记，设计不上手改。
本名册的普查域**限于 `crates/nai-storage`**，域外按 W125 口径不收并具名在册（`SHAPE_LIMITS`）。

## 五、门控（`cargo xtask check --profile full`）

**首跑在第一步 `cargo fmt --all --check` 即红**（21 s）——本轮新增的 `tests.rs` 行不符合 rustfmt 风格
（名册条目该展开成多行、`is_disjoint` 三行该换行、一处多余空行）。**这暴露了一个流程缺口**：先测后设计
之后没有**先跑 fmt**，于是 8 组注入验证都是在**非 fmt-clean** 的字节上做的（判据行为不受影响，
但「逐字节还原」核对的是那个非 clean 状态）。处置：`cargo fmt --all` ⇒ `--check` 通过（`FMT_OK`），
注入验证已完成并留证，**不重做**（重做不会改变结论，只会重复代价）；如实记在这里。

**二跑全绿**（`CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`，9m19s，`GATE_EXIT=0`），现测计数：

- 错误类扫描（`LNK1104` / `error[` / `out of memory` / `^error` / `FAILED`）：**0**
- `test result` 行 **97** 条 = **79** 个真实测试目标（`Running unittests|tests\`）+ **18** 个 `Doc-tests`
- Rust 合计 **1037 passed / 0 failed**（逐目标 `0 failed`，`grep -cE "test result:.*[1-9]… failed"` = **0**）
- Node 侧 **13** 个套件、**323 passed / 0 failed**（逐套件 `# fail 0` = 13/13）
- 前端 6 步全过（`[frontend] 全部步骤通过`）
- **真实 rustc 告警 0**（排除增量编译 lock 文件的宿主噪音）

**对照 W128**（**1028 passed / 0 failed**、**79** 个目标）：Rust **+9**，与本轮「+8 行为判据 +1 闭世界」
**逐项吻合**；目标数 **79 不变**——`storage_bounds.rs` 是 `src/tests/` 下的**模块**（挂在 lib 测试目标上），
不是新的独立测试目标。

⚠️ 本日志**不含** `GATE_OK` / `PREBUILD_OK` token（W127 已记：MUST NOT 把「没有该 token」判成失败）；
结论以 `GATE_EXIT=0` 与上表逐项现测为准。本轮未触及 `migration::VERSION`（`git status` 现测无迁移文件），
无升版成套六处；无新增依赖 ⇒ `Cargo.lock` **无改动**（现测）。


## 六、提交与推送

（待回填）

## 七、CI 观测

（待回填）
