# W111：清除副本从「字段」变成「动作」（§42.4 清理侧）

- 轮次：W111
- 日期：2026-10-15
- 变更：`openspec/changes/2026-10-15-w111-memory-purge-surface/`
- ADR：[ADR 0089](../adr/0089-memory-purge-surface.md)
- 前驱：[W110](2026-10-15-w110-memory-control-surface.md)（把读取面与遗忘落点接上控制面）

## 一、缝在哪（现测，先跑红再设计）

W110 把遗忘接上了控制面，但**清除侧仍是调用方在填空题**。立项时先写判据、先跑红：

```
test claiming_fully_erased_must_leave_no_payload_in_the_store ... FAILED
宣称已完全擦除 ⇒ 载荷 MUST NOT 仍在库里；实测仍读到
  "the record body that MUST be persisted verbatim, not a pointer"
```

请求体给 `purge_state: "complete"` + `claims_fully_erased: true` ⇒ 系统**接受**（200），
而 `memory_records.content` 里那句原文**一字未动**。

| 事实 | 取证 |
| --- | --- |
| 两个字段**全部来自请求体** | `crates/nai-control-api/src/lib.rs:786-819`（W110 时的行号） |
| 运行时只把两者**转手**给 `ForgetOutcome` | `crates/nai-runtime/src/memory.rs:230-237` |
| 判定只查**陈述内部自洽** | `crates/nai-kernel/src/evaluate/memory.rs:846-869` |
| 零清除动作 | 全仓无任何写 `memory_records.content` 的删除/置空语句 |
| 副本集合**空集** | 读取面直扫表；无记忆索引/缓存/远端同步；控制面与 runtime 零 `decision_log`/`audit_records` 写入 |

⚠️ **交付面自相矛盾**：W110 报告如实写着「副本清除未执行」，而端点却允许宣称已清完。

## 二、设计（详见 [ADR 0089](../adr/0089-memory-purge-surface.md)）

八条决策，要紧的四条：

1. **结论不进请求体**（破坏性）：`purge_state` / `claims_fully_erased` 删除，
   留同名按**未知字段**拒 `400`——静默忽略等于把缺陷换位置。
   `third_party_retention` **保留**：它是**事实**，只有调用方知道。
2. **`body_purged` 从存储读**：新增 `Store::memory_body_purged_at`，由「载荷真的被清过」
   得出，**不是**入参——写死 `true` 会让新拒因变成死分支。
3. **检查顺序由可达性决定**：Complete → body_purged → 留存。反过来排会让
   `ErasedClaimWithoutCompletePurge` **不可达**（用例随即红）。
4. **口径写进形状**：`erased.physical_bytes_erased` **恒 `false`**——置空一列不是介质级擦除。

## 三、实现

| 位置 | 改动 |
| --- | --- |
| `migrations/sqlite/0009-memory-purge.sql` | 新增：`memory_records.purged_at_ms`（可空） |
| `crates/nai-storage/src/store.rs` | `purge_memory_body`（清 content、**只置一次**、保留行）、`memory_body_purged_at`（读事实） |
| `crates/nai-kernel/src/evaluate/memory.rs` | `ForgetOutcome.body_purged` + 拒因 `ErasedClaimWithoutPurge`（`ALL` 23 → **24**）+ 判定顺序 |
| `crates/nai-runtime/src/memory.rs` | `purge_memory` 执行体、`PurgeReport`、`CopySite`/`COPY_SITES`、适配器改调清除入口 |
| `crates/nai-control-api/src/lib.rs` | `MemoryForgetRequest` 收窄为 3 字段、`MemoryPurgeReport`、未知字段拒 `400`、响应带系统结论 |
| 版本号成套**五处** | `VERSION` 9 / `SQL` 数组 / `backup.schema.json` const 9 / Rust 契约样本 9 / **Node 备份样本 9→8** |

`Store` 侧刻意的三条：`content=''` 与 `purged_at_ms` 分两条 UPDATE（先清载荷再记时刻，
任一步失败都不会留下「已记时刻但载荷还在」的行）；已撤销**照样能清**（阻断不是拒绝清除的理由）。

## 四、测试

| 文件 | 总数 | 本轮新增 |
| --- | --- | --- |
| `crates/nai-control-api/src/tests.rs` | **29** | **+3** |
| `crates/nai-runtime/tests/memory.rs` | **29** | **+6** |
| `crates/nai-kernel/tests/memory.rs` | **23** | 改编 3 处（新增 1 条拒因的构造） |
| `crates/nai-storage/src/migration.rs` | — | **+1**（0009 升级），并**修掉**一条漂移用例 |

runtime 侧六条：零执行宣称被具名拒绝**且载荷原样还在**；清除后载荷为空、行仍在、
结论自得；重复清除时刻不变（20ms 实差）；第三方留存 ⇒ `blocked_third_party`；
清除失败时读取侧**已**阻断；副本清单闭世界（逐字清除体 + **代码派生**的落点集合 +
空集被清点）。

⚠️ **一条弱判据如实登记**：`assert_eq!(report.copy_sites_cleared, COPY_SITES.len())`
两边同源，属**自指比较**，它只能钉「报告与清单一致」，**不能**钉「清单是对的」；
真正抓住错误清单的是闭世界判据的 ①②（I3 实测证明）。

⚠️ **W109 遗留的漂移在升版时暴露**：`version_eight_upgrade_...` 原写 `VERSION - 1`，
`VERSION` 升到 9 之后它**悄悄改测了 v8→v9**（用例名成了不实陈述）。
已钉死 `const MEMORY_REVOCATION: usize = 8`——**这正是 MEMORY 里那条硬规则的第二次兑现**。

## 五、非空洞注入验证（真改文件 + 真跑 + 逐字节还原）

| 组 | 注入 | 结果 |
| --- | --- | --- |
| I1 | `purge_memory_body` 只记时刻、**不清 content** | **3** 红 |
| I2 | 控制面把 `purge_state`/`claims_fully_erased` 放回白名单（=重新接受结论） | **3** 红 |
| I3 | `COPY_SITES` 登记一个**不存在**的清除体 `purge_memory_index` | **1** 红 |

共 **7** 红，每组 ≥1；`sha256` **逐字节**还原 ✓（三文件 `sha256sum -c` 全 OK），无 `INJECT` 残留。
注入全程用**编辑工具**（Python 读写往返会统一换行符、令 `sha256` 失效）。

I3 的信号最值得记：清点逻辑**照报** `copy_sites_cleared = 1`（它数的是清单长度），
只有闭世界判据的「清除体必须逐字存在」抓住了它 ⇒ **计数正确 ≠ 清单正确**。

## 六、门控与 validate

### 6.1 全量门控

`cargo xtask check --profile full` ⇒ **`EXIT=0`**（限并发 `RUST_TEST_THREADS=1`）：

- Rust：**95** 个测试目标 / **950 passed** / **0 failed**
- Node：13 个根 / **319** / **0**
- `error[` / `LNK1104` / OOM / `DeadlineExceeded` / `not ok ` 计数**全为 0**

对照 W110（**940**）⇒ **+10** = `nai-control-api` **+3**（26 → 29）+ `tests/memory.rs` **+6**
（23 → 29）+ `migration.rs` 的 0009 升级用例 **+1**，与第四章**逐项吻合**。
三处都落在**既有测试目标内** ⇒ Rust 目标数与 Node 根数**均不变** ⇒ **无需** `engineering_checks` 登记。

⚠️ 门控**第一次跑红在 `cargo fmt --check`**（本轮改动破坏了格式），`cargo fmt --all` 后**第二次**绿。
按纪律这属于「一次就绿」的反例，**不据此推断**什么，只如实记：本次门控跑了 **3 次**
（fmt 红 → 绿 → 归档后复跑）。

### 6.2 OpenSpec validate

| 校验 | 结果 |
| --- | --- |
| 变更级 `--strict` | **`is valid`** |
| `--specs --strict`（归档前） | **43 passed / 0 failed** |
| `--changes`（归档前） | **1 / 0** |

归档 `2026-10-15-w111-memory-purge-surface` ⇒ `local-control-surface` **+2 added / ~1 modified**、
`local-persistence` **+2 added**、`runtime-composition` **+1 added**（合计 **+5 / ~1 / −0**）。
归档后**立刻**跑的两条：`--archived` ⇒ **109 passed / 0 failed**（W110 为 108 ⇒ **+1**）；
`--all --strict` ⇒ **43 / 0**。

⚠️ `--archived` **首版报 1 红**（`1 incomplete task (19/20)`）：§6.4 把
「归档 + 提交 + 推送 + CI + 回填」捆成了一个 checkbox，而**归档本身要先完成**、
**回填需要 run id** ⇒ 那是一条**结构上无法在归档前勾**的任务。已就地订正为**流程项（非 checkbox）**，
复验 109/0。（这是这条纪律的**第三次**兑现，W110 同因。）

⚠️ `local-control-surface` 走的是 **MODIFIED**（修订 W110 的「Forget blocks the read side
first and fails by name」），因为请求体形状变了却不能让旧 Requirement 继续描述旧形状。

归档改动了活规格 ⇒ 按纪律**复跑门控**（第三次）：`EXIT=0`，计数与 §6.1 **逐项相同**
（95 目标 / 950 passed / 0 failed；Node 13 / 319 / 0；故障痕迹 0）。

## 七、如实披露、遗留与边界

### 7.1 如实披露

- **只承诺逻辑擦除**：本仓未做介质级擦除，响应恒带 `physical_bytes_erased: false`。
  「不宣称已完全擦除」由**构造**保证（`PurgeReport` 的字段由执行体写死）。
- **副本清单当前为空集**：这是**被守卫的事实**（代码派生的落点集合恰为 `memory_records`），
  不是「还没查」。将来加了记忆索引/缓存/远端同步，MUST 登记清除体，否则闭世界判据会红。
- **判定顺序是可达性驱动的**：不是风格选择，见 ADR 0089 决策 4。

### 7.2 遗留

1. 远端留存清除（§24.4 知识链那套 `RemoteRetention`，与记忆记录不共用落点）。
2. 介质级擦除（明确**不做**，除非产品要求并能验证）。
3. §42.5 Pro 跨会话、桌面 UI 呈现（承自 W110）。

### 7.3 边界（本轮**不做**）

- 不改 `admit_write` / `read_records` 的任何判定规则。
- 不删行（tombstone 保留），不引入缓存层，不做 `VACUUM`。

**H03 / H04 的 registry `status` 仍为 `pending`**——本轮闭合的是「清除侧：字段 → 动作」，
**不是验收达标**。

## 八、归档、提交与 CI（**待回填**）

归档、`--archived`、提交、推送与 CI 结论在本节回填。