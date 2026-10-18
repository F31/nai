# Tasks: W140 — 媒体因果链：把「清队列」改成点名并报告

## 1. 勘定（先量，再动手）

- [x] 1.1 复核 W139 六条结论与落点 (e)；确认 `MediaGate::with_epoch_floor` 的生产装配点只有
      `crates/nai-runtime/src/composition.rs:160` 与 `apps/output-keeper/src/lib.rs:99`
- [x] 1.2 现测签名面：`poll(&mut self, now: Instant)` 无 `utc_ms` ⇒ 取消**不能**依赖时钟，
      故 `recycle_at_ms` 必须在登记时写入（省下 11 个既有调用点的改动）
- [x] 1.3 现测 `enqueue`/`consume` 的调用点：全仓只在 `tests/gate.rs` ⇒ 这两条登记点在**生产路径上没有调用者**
      （如实登记，MUST NOT 写成「已接通」）
- [x] 1.4 现测 registry 88 条验收条目：**无**对应「媒体因果链/级联取消」的条目 ⇒ 本轮不标任何条目为 `tested`
- [x] 1.5 现测 xtask 只枚举 JS 判据、不枚举 Rust 测试文件 ⇒ 新增 `tests/lineage.rs` **无需**在
      `engineering_checks` 登记
- [x] 1.6 现测「宽限」的既有参考实现：全仓 `crates/**.rs` 中 `grace|Grace` **零命中** ⇒ 零先例，
      三种退化里的这一条没有现成形状可抄

## 2. 定义性对象

- [x] 2.1 `crates/nai-media-core/src/lineage.rs`：`LineageId` 三元组 / `FacetKind` 五值闭集 /
      `NodeState` 四值 + `TERMINAL_STATES` 单一派生 / `LineageNode` / `LineageRef` / `LineageReport`
      （`count()` 由 `manifest.len()` 派生）/ `LineageError` 三值闭集 / `LineageRegistry`
- [x] 2.2 零新增依赖（仅 `std::collections::BTreeMap`）；`RECYCLE_GRACE_MS` 具名常量

## 3. 门控接线

- [x] 3.1 `MediaGate` 增 `lineage` / `cancellation` / `reclamation` 三个字段，`with_epoch_floor` 初始化
- [x] 3.2 `arm`：`cancel_scope()` → `advance_epoch` → 为每个 `allowed_utterance_ids` 登记 `UtteranceRow`
      → `reclaim(utc_ms)`；`queue.clear()` 保留（队列仍是顺序权威）
- [x] 3.3 `poll` 失败分支：`cancel_scope()` 并把报告存入 `cancellation`
- [x] 3.4 `enqueue` 登记 `PendingGate` 子节点（在容量检查之后，故重复入队不变语义）+ `reclaim(utc_ms)`
- [x] 3.5 `consume` 释放子节点；队列点名而 lineage 无在途子节点 ⇒ `MediaError::Stale`
- [x] 3.6 读取面：`lineage()` / `take_cancellation()` / `take_reclamation()`
- [x] 3.7 既有 61 个用例全绿（签名零变更，无调用点改动）

## 4. 判据与注入

- [x] 4.1 `tests/lineage.rs` **7** 条：facade 闭世界 / 终态划分 / 取消点名且计数==清单 /
      宽限是值 / 作用域与父拒绝 / 释放报 0 / 跨代回收有界
- [x] 4.2 `tests/gate.rs` 新增 **4** 条接线用例：arm 声明可恢复行（空清单也报）/
      重新布防点名上一代 / 静音点名两个节点 / enqueue 登记与 consume 释放
- [x] 4.3 注入 **4** 组全部真红：I1 取消只改状态不入清单（3 条红）｜I2 终态掉「已回收」一格（1 条红）｜
      I3 回收忽略记录值（1 条红）｜I4 `enqueue` 不登记子节点（3 条红，含一条既有用例）
- [x] 4.4 4 组还原后 `sha256sum -c` 两个文件逐字节 `OK`
- [x] 4.5 `cargo fmt --all --check` clean；`cargo clippy -p nai-media-core --all-targets` 零告警

## 5. 文档

- [x] 5.1 OpenSpec 变更 `w140-media-causal-lineage`（proposal / spec delta / tasks）
- [x] 5.2 ADR 0114（落点 (e) 的选择与被否决的 (a)–(d)、三条判据形状、诚实边界）
- [x] 5.3 报告 `docs/reports/2026-10-18-w140-media-causal-lineage.md`
- [x] 5.4 `docs/development-plan.md` 同步

## 6. 收口

- [x] 6.1 门控 `check --profile full`（`CARGO_INCREMENTAL=0` + `CARGO_BUILD_JOBS=1`）取 `GATE_EXIT=0` 与计数
- [x] 6.2 归档 + `validate --all --strict` + `validate --archived`
- [x] 6.3 一次 commit + push（SSH）
- [x] 6.4 CI 观测三件齐（`cb=` 纳秒 + `no-cache` + 回读 `head_sha`）
- [x] 6.5 回填报告门控与 CI 段，清理临时文件
