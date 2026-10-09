# Proposal

## Why
D12、V3.5 §9 要求完整模型与运行时兼容证据。现有模型 JSON Schema 只有声明结构检查，需要在取得真实模型前建立可验证的内容完整性边界。

## What Changes
- SDK 增加模型清单检查：唯一子图、required_graph_ids 闭包、规范路径、全部声明文件的内容哈希。
- 精确匹配调用方指定运行时矩阵，不自动 CPU 回落；检查有界、可取消、有 deadline。
- 结果仅表示声明及内容完整性，不授予 G0–G3、Ready 或执行权限。

## Capabilities
### New Capabilities
- `model-package-integrity`: 模型声明闭包、内容哈希与运行时兼容检查。
### Modified Capabilities
无。既有 versioned-contracts 的结构校验保持不变。

## Impact
修改 nai-plugin-sdk；沿用 schemas/plugin/model.schema.json，无 schema 变更或新增依赖、常驻进程。D12 推理/导出与真实 G0–G3 依赖模型及目标机，本轮不宣称完成。
