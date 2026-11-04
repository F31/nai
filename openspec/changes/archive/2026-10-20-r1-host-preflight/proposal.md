# Proposal

## Why

首次设置能在界面内完成配置与启动，但一键预检仍只能从命令行运行，桌面预检面板没有报告可读。顺序 3 的“输入 API 配置→预检→首句”缺了界面内产生报告的一步。

## What Changes

- 宿主新增 `run_host_preflight`：用已保存设置运行 `nai-engine --preflight`，可选连通探针，返回报告轴。
- 设置面板加入“运行预检”，结果逐轴展示；不阻塞连接，也不把 `not_ready` 说成通过。
- 预检报告仍写在数据根，连接后由既有控制路由读回，避免第二份报告语义。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `desktop-shell`: 宿主可在界面内运行预检并如实展示结果。

## Impact

顺序 3、A12、O01；V3.11 §9、§29。无新增依赖、新增常驻服务或数据格式；仅多一个窄宿主命令。
