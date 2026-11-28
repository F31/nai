# V5.6.1 验收注册表

本目录是 V5.6.1 单引擎基线的唯一实际验收来源。

- 注册表：`tests/v561/registry.yaml`
- Schema：`schemas/tests/v561-registry.schema.json`
- 检查器：`node scripts/check-v561-registry.mjs`
- 旧 V4.8 注册表：`tests/registry.yaml`，只读归档，不再授予本基线通过。
- 旧 V4.8/V5.6 的 ACC/INV/任务编号不继承；新项必须使用 `ACC-V561-*`。
- 初始状态允许全部 `pending`；`passed` 必须有证据，`blocked` 必须有原因。
- M0/M1/M2/M3 分别对应目标机基线、30 分钟、2 小时、4/8 小时；未通过前不得跳级声明。
