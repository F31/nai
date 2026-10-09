# Design

## Context
既有 Session 命令支持 CAS/epoch/幂等，MediaGate EmergencyHandle 是独立原子信号。control.yaml 仅有契约、TS 类型，桌面尚未连接。V3.5 §12/§22 与 D14 为本轮依据。

## Decisions
- 本轮只实现 GET health/runtime/status/session 与 POST mute/takeover；其余路由明确返回 CapabilityAbsent，不编造预检/插件 Ready/审批。
- HTTP 服务复用锁定 Hyper/Tokio：仅接收 loopback listener，最多 16 个并发连接；每连接总期限 5 秒，单请求体最多 4 KiB，Session 状态锁使用 try_lock，不无限等待。服务显式启动，关闭即释放监听。
- 可信 root 注册固定 Session、EmergencyHandle 和一个作用域权限令牌；token 仅保存 SHA256，至少 32 字节，存在期限及可撤销标记。浏览器 token 仅组件内存，URL/日志/持久配置不携带 token。身份/权限在读取前及命令执行前复检。
- 增加 SessionCommand device_id/seq；设备绑定到令牌，seq 必须单调，新命令先外层有界 CommandLedger 去重再验序号与调用 Session。返回原 ACK，改变同 ID 内容拒绝。TTL/deadline 与容量受限；失败不推进序号。
- 静音/接管在鉴权、权限、scope、体结构校验后先撤销媒体，才 try_lock/CAS/幂等；状态冲突仍保持媒体拒绝。独立 EmergencyHandle 保持本机优先，网络容量或失联时不保证远程 P0 投递。
- ACK executed 仅表示 Session.command 执行，runtime safe_message 明确没有设备 ACK。示例程序创建真实空 Session/MediaGate，无插件、设备或模拟事件，不让其 Ready。
- OpenAPI 先修改/生成；新增生成的 control_http::SCHEMAS_JSON，以 allOf 扁平化/闭合顶层字段服务消费。API 请求/响应使用 serde_json Value+生成 schema，不手写跨语言 DTO。
- CORS 只允许明确配置的本机桌面 dev origin；预检 OPTIONS 不暴露数据。请求带 query 一律拒绝。Production protected IPC/Tauri、TLS、手机配对、全场启动/审批与持久化在后续实现。

## Risks / Trade-offs
- HTTP 工程入口不是 Windows protected IPC → 显式示例启动，不变更 Local 正式默认，文档记录生产缺口。
- UI 关闭不结束 Session → 页面断连不发 Stop；示例进程寿命由操作员维护，本轮未做生产媒体宿主。
- 命令响应丢失/状态冲突 → UI 不自动重播新命令；刷新真实状态，幂等测试保证同 ID 重试。
- 撤销信号不等于声卡静音 → 界面与 ACK 说明控制层范围，设备认证未执行。

## Verification
真实 loopback HTTP 测试覆盖认证/权限/过期、会话隔离、seq/CAS、幂等、体大小、未知字段/CORS/查询 token、状态锁忙时先静音；frontend build/typecheck 与客户端行为测试覆盖响应结构、失败拒绝、精确状态/命令序号。浏览器人工操作与 Windows 原生验收另记录，不能以编译冒充 UI 实测。portable/contracts/OpenSpec 验证。
