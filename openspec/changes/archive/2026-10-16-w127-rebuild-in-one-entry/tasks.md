# Tasks: W127 — 从决策日志重建装配，以**条目**为单位（H02）

- [x] 1. 先跑会红的判据：`tests/registry.yaml` 的 H02 标题即口径来源；判定面/接线/基线原文逐一核实（含 `ordered_fragments` 与 `entry` 的关系）
- [x] 2. runtime 侧新增 **2** 条会红的判据（跨两条目分裂、承载条目是超集），首跑确认**红**（实测 `rebuildable = true`、零拒绝名）
- [x] 3. kernel 判定面：`entry_carries_assembly`（标识多重集、不计顺序）+ `rebuild_assembly_from_entries`（存在性 + 并集 + 整链派生 `unlogged_ids`）+ `RebuildRejection::NotInOneEntry`（`ALL` 6 → 7）+ `version_rejections` 抽为单一来源
- [x] 4. kernel 判据 36 → **40**：多重集/顺序无关、超集不是本装配、分裂被具名拒且不得报未落日志、存在一个承载条目即足够且不吞掉 `order_differs`、空链两名并存；`ALL` 总账用例补上新变体的构造
- [x] 5. runtime 接线：`rebuild_assembly_from_log` 去掉 `flat_map`、逐行作为独立条目；删去「跨行合并」⚠️ 披露；写路径**不动**并在设计里说明为何不该统一
- [x] 6. runtime 判据 **25 → 27**；非空洞守卫改造：`not_in_one_entry` 走**读回面**取样，注释写明链级/单次写入级的不对称
- [x] 7. 注入验证：每条新守卫各自必须变红（多重集退化为集合、超集视为承载、失败分支清空 `rebuilt`、`unlogged_ids` 对空池派生、选行改按确切顺序），逐字节还原
- [x] 8. 性质判定：`DeadlineExceeded` 夹具瞬态 ⇒ 限并发复跑全绿，如实登记（**不改代码**）
- [x] 9. spec：`runtime-composition` 的 `An assembly is rebuilt from the persisted chain alone` 走 **MODIFIED**，补 2 个 Scenario
- [x] 10. registry：`H01` / `H02` notes 就地追加本轮段落，**status 均维持 `pending`**
- [x] 11. 文档：ADR 0105 + W127 报告 + `docs/runtime.md` / `AGENTS.md` / `docs/development-plan.md` 活文档 1:1 订正 + 历史记录（ADR 0082、ADR 0084、W106 报告、已归档 W106 变更）**追加勘误段**
- [x] 12. 全量门控 + 归档 + 两条 validate + 一次 commit + push + CI 观测

归档 → `validate --archived --strict` → commit/push → CI 双平台观测 → 回填报告。
