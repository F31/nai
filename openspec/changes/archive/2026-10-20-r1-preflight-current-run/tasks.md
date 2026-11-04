## 1. 规格

- [x] 1.1 建立 proposal/specs/design/tasks 并严格校验。

## 2. 实现与接线

- [x] 2.1 宿主预检串行准入、旧报告失效、失败清理和有界严格读取；错误不回显原始输出（独立壳 lib 11/11）。
- [x] 2.2 增加旧报告、进程失败、超时、缺失/损坏/超界报告、并发与有效拒绝的意义测试；真实 engine 重复运行及原生控制读回验证（集成 3/3）。

## 3. 验证与归档

- [x] 3.1 运行独立壳测试/clippy/fmt 与根 full 门控，如实记录环境和结果（full EXIT=0，Rust 1209/0，Node 450/0；独立壳 11+3，smoke EXIT=0）。
- [x] 3.2 更新 docs/development-plan.md、交付计划及 docs/reports，保持验收状态不变。

## Workflow follow-up

- 完成后归档并验证主规格、归档与归档后回归，结果写入本轮报告。
