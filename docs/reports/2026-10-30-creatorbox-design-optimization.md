# CreatorBox 启发的 V5.2 设计与规格优化

设计目标已统一到 `docs/v52-agent-avatar-creatorbox-optimization.md`，并在 V5.2 首页建立增补入口：真人视频＋语音口型优先，统一智能体决定讲解和互动，简化形象/声音/主题操作；分离私有准备、OBS 实际音画与平台开播。记录按需加载、预计算/流式差异、受控输出、真实回执、接管与性能对照。CreatorBox 的公开描述只作参考，可选服务的许可、实现和实测未确认；独立 Python 服务不得冒充原生 ONNX 或绕过当前正式包约束。

更新 `v49-realistic-avatar` 的 proposal/design/spec/tasks 与 `v52-personal-live-workbench` 的 design/spec/tasks，补入视频导入、失败、样本时轴、外部后端失联边界、OBS 音视频及智能体统一场次、用户体验和按同素材分级验收。新任务均未勾选，不变更既有 V4.8 验收注册表，不代表真实画面、公屏或 OBS 验收通过。个人模式涉及原审批和预算规格的调整仍依 ADR 0191 先裁决和迁移，用户数据及既有守门保持现状。

验证：使用项目缓存中的 Node 22.23.1 与 pnpm 8.15.0，`openspec validate v49-realistic-avatar --strict --no-interactive` 和 `openspec validate v52-personal-live-workbench --strict --no-interactive` 均通过。本次只修改设计文档和 OpenSpec，没有运行模型、触碰用户数据或更新安装版。
