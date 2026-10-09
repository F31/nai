## Why
D14 只实现了 loopback 状态/静音/接管，审批路由（GET /approvals、approve/reject）仍是 OpenAPI 占位。V3.6 的 O02/S02 需要操作员能查看待审批候选并批准或拒绝，且审批必须有上限并记录决策日志。

## What Changes
- storage 增加待审批候选查询（按 workspace/session/epoch 过滤、有界列表、满页明确提示），并增加拒绝推进（候选标记 Blocked）与审批读取。
- control-api 的 GET /approvals、POST /approvals/{id}/approve 与 /reject 接线并实现；鉴权、作用域、候选哈希、epoch/revision、TTL 与决策日志写入。
- 审批上限：单 session 待审批候选与审批命令数量有界；超限拒绝并返回明确原因。所有写入先检查权限与作用域，再更新状态。
- OpenAPI 只把这三个操作标记为 implemented-loopback；其余路由保持未实现。

## Capabilities
### New Capabilities
- `approval-workflow`: 待审批列表、批准、拒绝与有界审批闭环。
### Modified Capabilities
无。沿用既有 approval.read/approval.write 权限、候选哈希与决策日志。

## Impact
修改 schemas/openapi/control.yaml（impl 标记及元信息字段）、nai-storage（查询/拒绝/读取与事务日志）、nai-control-api（路由接线）、文档与测试；control-api 复用已有 nai-storage 和测试 tempfile，不新增外部运行依赖、crate 或进程。真实候选生成/生产 root 接线、GUI 确认与手机审核不在本轮。
