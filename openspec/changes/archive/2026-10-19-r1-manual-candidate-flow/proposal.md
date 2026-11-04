# Proposal

## Why

R1 操作者目前不能把手工问题送入真实 API 并在同一桌面审批结果。演练脚本、真实 Provider 冒烟和审批后台各自存在，但缺少应用编排。

## What Changes

- 接通已有 manual-events/events HTTP 契约、单调命令账本与有界生成任务。
- 显式本机场景清单提供知识、Provider 配置与凭据环境变量引用；通过真实存储检索、装配、API、守门、候选入库交付最终文本。
- 桌面提供输入、任务状态与现有审批列表，取消使用既有静音/接管撤权。
- 缺配置、无知识、local-only、失败、过期或换代均具名，不替换成演练桩。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `local-control-surface`: 手工事件从接收到生成候选及审批的实际路径。

## Impact

对应既有 D08/D09/D10/D14 接缝与 R1/O02、H01/H02/H15，依据基线 §5、§6、§41、§47。不新增 D 编号、验收状态或常驻服务。修改 HTTP 实现状态并重新生成；场景清单使用 JSON Schema。复用既有依赖，无新增第三方包。止于候选与人工审批，不宣称媒体播放或真实付费验收。
