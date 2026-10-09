# Tasks

## 1. 基线与规格
- [x] 1.1 起草并审阅 ADR 0015（V3.6 升基线、勘误裁决、命名保持 nai-*）。
- [x] 1.2 strict 校验本轮 proposal/specs/design/tasks。
## 2. 验收注册表
- [x] 2.1 建 `tests/registry.yaml` 与 `schemas/tests/registry.schema.json`，登记全部验收项与 release_line。
- [x] 2.2 自建 D01–D14→新编号映射与回填规则，旧 T/E/U 仅追溯。
## 3. xtask 与集成
- [x] 3.1 xtask 增加 `check --registry` 一致性检查并接入 portable 检查。
- [x] 3.2 更新 design.md/AGENTS/config.yaml/开发计划，运行验证并归档。

## Workflow follow-up

- 完成后归档本轮 admission-registry 增量，校验主规格与归档。后续切片任务以 registry 验收为准。
