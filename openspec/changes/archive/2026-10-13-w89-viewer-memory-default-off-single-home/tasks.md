# w89 任务

- [x] 核实 PD-31 的推荐与仓库真实状态（`memory.rs` 消费点／配置面 `session_defaults`／基线 §5.5 与 §48.7／W57 报告）
- [x] 裁清：能力开关收成**唯一一处**判定；写入面补执行体；拒因**复用**不新造
- [x] 建 change 目录与 OpenSpec 规格差量（`agent-harness`：1 ADDED）
- [x] 实现 `viewer_memory_gate` + `read_records`／`viewer_record_allowed` 改引 + `admit_write` 接策略
- [x] 集成测试：改调用点，新增三组判据（写面被拒／顺序断言／启用后通过且非观众不受影响）
- [x] 非空洞注入：改前/改后对照（改前 0 拒 → 改后 1 具名拒）；把 gate 移出后坏值必须静默通过
- [x] 台账 `PD-31` 迁 `decided_in_repo` + `landing`，就地订正「空集」的指代；分布断言 4→3 / 1→2
- [x] 写 ADR 0067、ADR 0054 追加勘误段、同步活文档（AGENTS.md / development-plan.md / kernel.md）
- [x] 报告 + 全量门控 + 归档 + 两条 validate + 提交推送 + CI 观测 + 回填
