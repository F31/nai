## Context
main.tsx 的审批行没有使用服务端 display_text。ControlClient.planUpgrade/fetchJob 已有真实 HTTP 接线，upgrade.ts 的成本与状态呈现函数也已存在，但页面没有调用。

## Decisions
1. React 页面直接消费既有客户端与呈现函数，不新建 API、成本算法或台账。
2. 包清单以有界 JSON 文本输入；解析后调用 planUpgrade。作业查询必须显式输入 id，不从成本查询虚构作业。
3. 每次查询先清理对应结果；连接切换、断开清理全部升级结果。异步回包复用既有连接 generation 防止旧响应覆盖新连接；静音可抢占查询。
4. 无安装器时只提供成本和状态查询，明确执行/回滚不可用，不渲染假执行按钮。
5. UI 测试覆盖真正使用的请求流程和错误清理；工程 typecheck/build 不替代浏览器渲染或 Windows 验收。

## Verification
运行既有 UI、desktop、frontend gate，控制面升级相关测试，以及匹配的工程 profile/OpenSpec 校验。报告记录实际结果和既有失败，不以新增判据数作为交付目标。
