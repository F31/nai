# Tasks

## 1. 规范和契约

- [x] 1.1 完成 proposal/specs/design/tasks 并通过严格校验。
- [x] 1.2 增补登记/冻结/结果 Protobuf 和 binding-plan Schema，重新生成并验证无漂移。
- [x] 1.3 SDK 实现同源清单验证/转换，测试范围、未知字段、OS/host及不兼容拒绝。

## 2. 内核登记与解析

- [x] 2.1 实现有界/可取消 registry、独立准入和循环拒绝；通过不污染注册/unknown inproc测试。
- [x] 2.2 实现会话权限/健康 TTL、依赖闭包和显式同域主备；测试歧义、撤销、顺序和跨域拒绝。
- [x] 2.3 实现必需/可选/硬门控的 Ready 判定与 CapabilityView；测试guard/profile/auto前置和不启动无关实现。
- [x] 2.4 实现冻结版本快照/当前复检/更新限制；测试跨会话、篡改、失效及不热换版。

## 3. 验证与交付

- [x] 3.1 实现 kernel suite/CI 和新DTO三语言互通；通过kernel/SDK/contract及Windows可用编译检查。
- [x] 3.2 更新ADR/操作指南、计划/报告/README/AGENTS；运行portable、前端与OpenSpec严格校验。

## Workflow follow-up

- 验证后归档合并主规格，并验证全部规格/归档任务。
