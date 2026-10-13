# w90 任务清单

- [x] 核实配置面的真实消费状况（非测试/非生成源码扫描；形状收紧以排除数组字面量）
- [x] 逐段核实 [C] 的成因（构造器注入 / 硬编码默认 / 面无代码 / 同名不同物 / 无消费者 / 待裁定）
- [x] 建立 OpenSpec 变更 `w90-config-surface-consumption`（proposal / design / spec 差量 / tasks）
- [x] 实现 `scripts/contracts/config-consumption.mjs`（派生 + 两张台账 + 四条判据）
- [x] 在 `tests/contracts/schema.test.mjs` 接入判据（`xtask` 零改动）
- [x] 非空洞注入 F0a–F12（含真跑改前/改后对照与还原校验）
- [x] 写 ADR 0068 与活文档同步（`docs/kernel.md` / `AGENTS.md` / `docs/development-plan.md`）
- [x] 写轮次报告并跑全量门控（`check --profile full`）与两条 `openspec validate`
- [x] 归档、一次提交推送、单次 CI 观测并回填报告
