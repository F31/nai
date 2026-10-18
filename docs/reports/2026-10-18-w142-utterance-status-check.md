# W142 报告：utterance 取值域下沉到 schema（N1）

- 日期：2026-10-18
- 起点：W141 报告 §八 的 **N1** ——「数据库层面 `utterances.status` 无 CHECK 约束 ⇒
  直连数据库仍可写入任意整数」。
- 交付：`migrations/sqlite/0012-utterance-status-domain.sql`（新）、`tests/status_schema.rs`（新）、
  ADR 0116、OpenSpec `w142-utterance-status-check`。

## 一、勘定

**该列在 0001 里的原文**（不是转述）：

```sql
status INTEGER NOT NULL DEFAULT 1
```

—— 没有 CHECK、没有枚举。W141 让每条 SQL 的守卫都由常量派生，但**这条约束只活在 Rust 的
调用路径上**。本轮开工前现测：v11 的库上 `UPDATE utterances SET status=7` 成功。

**引用 `utterances` 的子表只有一个**：`evidence_links`（外键 `(workspace_id,utterance_id)`，
见 `0001-foundation.sql:11`）。这一点决定了重建能不能做 —— 见下。

**重建落在外键生效的夹缝里**。`store.rs:200-206` 的顺序是：先
`pragma_update(None,"foreign_keys",true)`，再 `migration::migrate(...)`。于是：

| 变体（实测，`sqlite3` + 真实迁移链 + 一条真实子行） | 结果 |
| --- | --- |
| 不加 defer，直接重建 | `DROP TABLE` 报 `FOREIGN KEY constraint failed` |
| 只加 `defer_foreign_keys=ON` | 行插回来了，`PRAGMA foreign_key_check` 是**空的**，但 `COMMIT` 报 `FOREIGN KEY constraint failed` |
| `defer_foreign_keys=ON` … 结束时置回 `OFF` | 成功：行与子行都在，`foreign_key_check` 空 |
| 在事务内 `PRAGMA foreign_keys=OFF` | **no-op**（设置后读回仍是 `1`）⇒ 迁移关不掉强制 |

延迟违规是**计数**而不是重新求值，所以那对 pragma 里缺任何一条都会在**有数据的库**上失败，
在空表上则完全看不出来。

**六处成套**本轮现测复核仍是六处：`migration::VERSION` / `SQL` 数组（含 `include_str!`）/
`backup.schema.json` 的 `db_schema_version` / Rust 契约样本断言（`tests/knowledge_publication.rs`）/
Node 样本 + 「上一版 MUST 被拒」的数字（`tests/contracts/schema.test.mjs`）/
`packages/contracts-ts/src/generated/config/backup.ts`。`packages/contracts-ts/dist/` **未入库**，
不属承载物。

## 二、落了什么

1. **`0012-utterance-status-domain.sql`**：整表重建 `utterances`，列顺序与 0001+0006 完全一致
   （`display_text` 在末位），`status` 上写
   `CHECK(status IN(1,2,3,4,5,6,11,12,13,14,15))`，重建前 `defer_foreign_keys=ON`、
   结束后 `=OFF`。行的复制是**逐列显式列举**（`INSERT ... SELECT` 不以 `*` 依赖列序）。
2. **域的取法**：`1..=5` ∪ `6` ∪ `11` ∪ `12..=15` = `classify()` **不返回 `Unassigned`** 的取值。
   于是 `0`、`7..=10`、`16..`、负数在存储里**不可表示**。
3. **六处成套**同步到 12（上一版被拒的数字 11 也 +1）。
4. **W141 那条判据的前提被改掉而不是关掉**：域外取值已写不进存储 ⇒ 样本无法构造。
   处置是拆成两处 —— 域内不可用（`11`/`12`/`13`/`14`/`15`）仍在 `status_domain.rs` 走真实
   `Store::approve`；域外拒绝搬到 `status_schema.rs`，判定位置从**调用时**前移到**写入时**。

## 三、判据

新增 **4 条**（`nai-storage` 90 → 94，正好 +4）：

| 位置 | 用例 | 钉的是什么 |
| --- | --- | --- |
| `tests/status_schema.rs` | `the_declared_set_is_pinned_and_its_complement_stays_outside` | 派生集合逐项在册 + **补集**（`0`、带上界之上、负数、`i64` 两端）在域外 |
| `tests/status_schema.rs` | `the_stored_check_clause_lists_exactly_the_declared_values` | 从 `sqlite_master` 读出建表语句，与 `classify` 派生的 `CHECK(status IN(...))` **逐字**比较；并断言重建的中间表没有残留 |
| `tests/status_schema.rs` | `the_schema_refuses_exactly_the_values_the_classifier_cannot_name` | 逐值 INSERT，接受 ⟺ `classify` 不返回 `Unassigned`；两侧都必须有样本；拒绝消息 MUST 含 `CHECK constraint failed` |
| `migration.rs` | `version_twelve_upgrade_enforces_the_utterance_status_domain_on_a_populated_database` | 带子行的库升级：行逐字保留、子行仍在、外键完好、升级**前**域外值可写、升级**后**被拒 |

两条设计取舍值得写明：

- **期望值从 `classify` 派生，不从迁移文件抄。** 判据读的是**存储里的建表语句**（真正约束写入的
  东西），不是迁移文件的文本。SQLite 调不到 Rust ⇒ 字面量必然存在两处；绑死它们的是这条判据的
  两个方向：改域不写迁移 ⇒ 红；改迁移的列表不改域 ⇒ 红。
- **拒绝的错误名必须是 `CHECK constraint failed`。** 否则「写不进去」可能只是撞上 `NOT NULL`、
  外键或 `STRICT` 类型 —— 那与取值域无关。

## 四、注入

四组，逐组真红、逐组可归因，还原后 `sha256sum -c` **4/4 OK**（基线是 fmt-clean 的最终字节）：

| 注入 | 改动 | 结果 |
| --- | --- | --- |
| I1 | 迁移里的 CHECK 列表放宽成 `0..=15` | **3 条红**：CHECK 子句逐字比较、逐值行为、升级用例的「域外被拒」 |
| I2 | 删掉 `PRAGMA defer_foreign_keys=ON` | **1 条红**：只有带子行的升级用例（`StoreError{sqlite constraint violated}`） |
| I3 | 保留 `ON`、删掉 `OFF` | **1 条红**：同一条用例，独立一次注入 |
| I4 | 把 `PENDING_BAND_HI` 5 → 7（「改了域不写迁移」） | **3 条红**：派生集合、CHECK 子句、逐值行为 |

⚠️ **I2 与 I3 落在同一条用例上，且失败消息同形**（`sqlite constraint violated` 是包装后的文案）。
也就是说：这条用例能**发现**两份 pragma 缺一，但**不能区分**缺的是哪一份 —— 归因靠的是两次独立
注入，不是用例的消息。如实记在这里，MUST NOT 说成「用例能分别定位」。

## 五、诚实边界

- **`Unassigned` 仍在 Rust 里**。`classify()` 的 `else` 分支保留（全域函数的完备性），
  升级后它**没有 SQL 写入路径**能造出来 —— 除非有人关掉约束。这**不是**「未分类态已消除」，
  而是「造不出未分类的数据」。
- **`11` 的语义仍然没裁定**（W141 的边界原样保留）。本轮只把「谁能写进这一列」收紧，
  没有回答「11 是什么」。
- **迁移只在本机 SQLite 上实测**（`sqlite3` 3.4x，Windows）。CI 的 Linux/Windows 两平台
  各自跑同一个迁移链，但都走同一份 `rusqlite` 内嵌 SQLite —— 这不是「两套实现互证」。
- **`tests/registry.yaml` 的 88 条里仍没有**对应「status 取值域」的验收条目 ⇒ 本轮
  **不**标任何条目状态，`O02` 之类的相邻条目也不因本轮改动而改变判定。
- 迁移是**有数据风险**的操作。本轮的证据是「一条真实子行」级别的，**不是**生产库规模；
  行数、页数、WAL 大小都没有测。

## 六、门控

`cargo xtask check --profile full` ⇒ **`GATE_EXIT=0`**（`CARGO_INCREMENTAL=0` +
`CARGO_BUILD_JOBS=1` + `RUST_TEST_THREADS=4` + 机外 `protoc 21.12`）：

| 口径 | 结果 |
| --- | --- |
| `test result: ok` 行数 / 通过数 | **102 行 / 1145 通过 / 0 失败** |
| Rust 目标 | **85** 个 `Running` + **18** 个 `Doc-tests` |
| Node 套件（`# pass` 行） | **15** 套 / **367 通过 / 0 失败**（含 desktop `test` 与 `test:ui`） |
| 前端 | 7 个步骤全过（`typecheck` ×2、`build` ×2、`test`、`test:ui` 71/71），末尾「工程基础，非渲染/人工 UI 验收」 |
| `LNK1104` | **0** |

**总数对得上**：W141 是 1141 passed，本轮 **1145** ⇒ **正好 +4**，与新增判据数一致
（既有用例零回归的算术证据）。

**中途的间歇 `LNK1104`（非代码缺陷，如实记录，且本轮多出两条新事实）**：前两次门控分别在
`render-fcf1e0bea8ec4327.exe`（`nai-avatar-2d` 的 `render` 测试）与
`nai_avatar_2d-bd75bdd586b3e252.exe`（该包 lib test）上命中；`cargo test --no-run` 预热
连跑 5 次又各自折在**不同**目标上（`accounting-…`、`control_http-…` 等）。判据仍取「**逐目标能过**」：
`cargo test -p nai-budget-core -p nai-avatar-2d` **9 + 9 全过**，说明源码与链接产物本身没问题。

两条**新测出的**操作事实（下一轮直接用，别再试错）：

1. **`cargo test --keep-going` 不存在**（cargo 明确回 `unexpected argument`，并提示 `--no-fail-fast`
   —— 那个只是「测试失败也继续跑」，不管编译）。要让构建「一次把能连的都连完」，
   得用 **`cargo build --workspace --tests --keep-going`**。改用它之后**第一次就 `EXIT=0`**，
   随后门控一次绿。
2. **单包预构建不能替代 workspace 预构建**：`cargo test -p nai-contracts --test control_http --no-run`
   产出的是 `control_http-cedb531209e1bc96.exe`，而门控要的是 `control_http-c2bba549833113e3.exe`
   —— 同一目标、不同哈希（`--workspace` 下的 feature 合并）。所以「单包能过」只能当**归因证据**，
   不能当**预热手段**。

## 七、CI 观测

推送 **`b3ccab4`**，运行 **`39098953192`**，回读 `head_sha` =
`b3ccab4795c1b0ae4273a7a679db8cabcd255612` 与推送 SHA **完整 40 位一致**
（三件齐：`cb=` 纳秒唯一参数 + `Cache-Control: no-cache` + 回读 `head_sha`）。

| 平台 | 结论 | 步数（API 口径） |
| --- | --- | --- |
| `foundation (ubuntu-24.04)` | `completed / success` | 20 |
| `foundation (windows-latest)` | `completed / success` | 20 |

逐平台看完**整条步骤序列**。非 success 项共 4 条（去重后 3 个步名），全部是 `skipped` 且逐条核到
`if` 条件：

- 步 8「Install Linux C++ protobuf runtime」（`foundation.yml:42` `if: runner.os == 'Linux'`）⇒ 仅 Windows 跳过；
- 步 14「Full three-language contract suite (Linux C++ toolchain)」（`foundation.yml:109` 同条件）⇒ 仅 Windows 跳过；
- 步 11「Publish the gate failure summary as annotations」（`foundation.yml:79` `if: failure()`）⇒ 两平台都跳过（门控没失败）。

步号从 16 跳到 30 是 `if` 未触发的步**不生成记录**（既有口径，MUST NOT 拿 `jobs[].steps` 条数与
workflow 的 `- name:` 步数相减）。

## 八、具名登记（本轮发现但不做）

| 编号 | 事项 | 归宿 |
| --- | --- | --- |
| M1 | `0007`/`0011`/`0012` 三次整表重建各自重复了一段「重建纪律」（`defer` 的对、列的显式列举、索引/触发器重建）。今天靠注释与各自用例守住 | 抽出重建模板要等第四次重建出现，且模板会掩盖「这次要不要重建索引」的判断（0011 重建的是有触发器的表，0012 重建的是有子表的表，两者的坑不同） |
| M2 | `defer_foreign_keys` 的错误被 `StorageError` 包装成 `sqlite constraint violated`，看不出是外键 | 包装层要不要透传 SQLite 的具体约束名，牵动全仓错误分类，属独立一轮 |
| M3 | 本轮把「域外取值」从运行期判据搬到了写入期，`status_domain.rs` 的用例名随之改了；W141 报告与 ADR 0115 里仍写着旧名 | 已发布文档按「追加勘误」处理；本轮在同一次提交里给 ADR 0115 追加了勘误段，旧报告不改 |
| M4 | `graphify-out/` 里仍有 `db_schema_version: 11` 的陈旧生成物 | 生成物目录，不入判据；升版后是否重跑 graphify 留给后续轮次 |
| M5 | `target-check/`（12G，10-15 遗留 scratch）仍在 | 沿用 W140 N6 / W141 N5：显式 `git add -- <path>` 规避，是否写进 `.gitignore` 仍未裁定 |
