# w91 任务清单

- [x] 核实三态读取形状（index 父键锚 / serde 同文件结构体 / helper 辅助函数），三轮迭代各被实测推翻一次
- [x] 核实归因文件集与外部读者（`composition.rs` / `pipeline.rs`），并做 runtime 回归（10/4/23 与 W90 逐项一致）
- [x] 建立 OpenSpec 变更 `w91-config-key-attribution`（proposal / design / spec 差量 / tasks）
- [x] 扩展 `scripts/contracts/config-consumption.mjs`：多 schema 派生 + 归因集 + 三态形状 + 五条判据
- [x] 在 `tests/contracts/schema.test.mjs` 接入判据（`xtask` 零改动）
- [x] 非空洞注入 F1–F17（含 F1 与 F12/F13 真跑改前/改后对照与还原校验，`sha256` 逐字一致）
- [x] 写 ADR 0069 与活文档同步（`docs/runtime.md` / `AGENTS.md` / `docs/development-plan.md`）
- [x] 写轮次报告并跑全量门控（`check --profile full`）与两条 `openspec validate`
- [x] 归档、一次提交推送、单次 CI 观测并回填报告
