# W111 任务：清除副本成为动作（§42.4 清理侧）

## 1 判据（先红）

- [x] 1.1 现测并写会红的判据：`claiming_fully_erased_must_leave_no_payload_in_the_store`
      （先跑红，红因＝载荷仍在库里）
- [x] 1.2 现测副本集合：读取面直扫表、无索引/缓存/远端/日志留痕 ⇒ 空集

## 2 承载物

- [x] 2.1 迁移 `0009-memory-purge.sql`（`purged_at_ms` 可空）
- [x] 2.2 版本号成套**五处**：`VERSION` / `SQL` 数组 / `backup.schema.json` const /
      Rust 契约样本断言 / `tests/contracts/schema.test.mjs` 的备份样本
- [x] 2.3 `Store::purge_memory_body`（置空 content、只置一次、返回生效时刻、保留行）

## 3 执行体与判定

- [x] 3.1 `ForgetOutcome` 增 `body_purged`（由事实得出，**不是**入参）
- [x] 3.2 判定增一条：宣称已完全擦除而无清除 ⇒ 具名拒因
- [x] 3.3 `Composition::purge_memory`：先阻断、再清载荷、清点副本清单、得出结论
- [x] 3.4 `forget_memory` 参数收窄（不再接受调用方的结论字段）

## 4 控制面

- [x] 4.1 `MemoryForgetRequest` 收窄为 `{record_id, trigger, third_party_retention?}`
- [x] 4.2 响应带系统结论：`revoked_at_ms` / `payload_purged_at_ms` / `copy_sites_cleared` /
      `purge_state` / `erased{scope, physical_bytes_erased:false}`
- [x] 4.3 副本清单 `COPY_SITES` + 闭世界判据（逐字清除体 + 代码派生落点 + 空集被清点）
- [x] 4.4 装配处改调清除入口；权限仍为 `memory.forget`

## 5 测试与非空洞注入

- [x] 5.1 runtime 用例：清除后载荷为空、行仍在、只置一次、已撤销可清、空集被清点
- [x] 5.2 控制面用例：请求体带结论字段 ⇒ `400`；响应结论由系统给出；`physical_bytes_erased:false`
- [x] 5.3 注入三组（**用编辑工具**、`sha256` 逐字节还原）：
      ① 清除不置空 content 却报已清；② 控制面重新接受调用方的 `claims_fully_erased`；
      ③ 副本清单闭世界被绕过（新增落点不入册不红）

## 6 文档与闭环

- [x] 6.1 ADR 0089 + 报告 + `development-plan.md` W111 段 + `kernel.md`/`runtime.md` 就地订正
- [x] 6.2 registry `H03`/`H04` notes 追加 W111 段（`status` **仍 `pending`**）
- [x] 6.3 全量门控 `cargo xtask check --profile full` + 两条 validate
归档 / 提交 / 推送 / CI 观测 / 回填（**流程项，非 checkbox**：归档本身需要先完成，
而回填需要 run id ⇒ 这些步骤在归档那一刻**结构上不可能勾完**，捆成 checkbox 只会造一条假红）

## 7 流程项（**非 checkbox**）

提交 / 推送 / CI 观测 / 回填在归档那一刻结构上不可能完成 ⇒ 不写成任务项。