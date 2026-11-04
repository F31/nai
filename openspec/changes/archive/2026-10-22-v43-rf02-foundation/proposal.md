## Why
RF01只有能力检查，新线没有可持久工作区、角色、版本对象、会话或交付门。RF02须形成真实单写存储与私有ContentOnly交付基础，才能由RF03接备播业务；不能将旧库或旧批准搬入新线。

## What Changes
- 新foundation Protobuf/存储Schema，先生成再实现消费者；单独认证的有界命令，不信客户端actor。
- 新SQLite storage adapter/对象仓库：独占writer、工作区角色、不可变对象版本/CAS、事务审计/outbox、幂等与预算端口。
- runtime-core新增纯策略DeliveryGate/停止栅栏；短期许可绑定主体、会话epoch、对象hash/revision、目标、范围和单调期限；每次交付前复检。
- 新引擎foundation运行入口真实落盘/重启和私有文本卡投影；先提交dispatch状态再处理目标，崩溃或不确定结果不盲重发。投影Submitted不冒充UI呈现/完成。
- 定向Linux/Windows数据库、进程与三语言契约检查；更新计划/报告，不替换旧安装或调用付费API。

## Capabilities
### New Capabilities
- `workbench-foundation`: 新线单写owner、身份、版本存储和私有交付基础。

## Impact
V43-RF02；INV-01/02/03/04/07/09/14/20/26/27/28/29/34前置或适用范围。公开输出、真实Provider、产品包/节目与发布UI由RF03之后实施；当前不授予整项ACC通过。
