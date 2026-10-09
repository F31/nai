# ADR 0014：显式本机控制工程入口

- 日期：2026-10-08
- 状态：Accepted（工程验证范围）
- 基线：V3.5 §12/§22 与 D14；Local 正式默认仍为受保护 IPC。

## 决策

以 shared 层 nai-control-api 组装 Session 和 EmergencyHandle 的最小状态/撤权端口，使用明确启动的 loopback HTTP 示例及桌面客户端进行真实端到端验证。复用既有锁定 Hyper/Tokio，增加依赖声明但不增加自动启动的常驻服务。内核与媒体库不依赖 HTTP、Tauri 或具体插件。

OpenAPI 继续作为 HTTP 唯一契约源；生成 control_http::SCHEMAS_JSON，扁平化 allOf 并封闭顶层命令字段。SessionCommand 增加 device_id/seq，对 HTTP 消费者为必需字段，已有 Protobuf ControlCommand 不变。旧 HTTP 客户端缺字段失败，不隐式填入设备身份或序号。

本轮 root 注册单个固定 Session 与有期限/可撤销的设备凭据，服务仅保存 token 哈希。生产设备配对/身份存储/多会话管理不在该凭据原型内假装完成。

P0 在可信身份与有效结构/scope/TTL 检查后独立撤销媒体，状态锁忙或 CAS 失败不能恢复输出。ACK 只表明控制状态更新，真实音轨须独立确认。网页不是离线独立 P0 的替代。

## 后果

桌面可真实读取空 Session 和执行静音/接管，未接设备仍显示明确不可用。HTTP 工程服务不作为 Windows 正式包、手机 HTTP 部署或生产认证；Tauri/protected IPC、开播预检、审批/证据、配置与持久化等保持待实施状态。
依赖成本、资源上限、命令与测试入口见 [控制指南](../control.md)。
