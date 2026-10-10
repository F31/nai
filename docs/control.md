# 本机控制面（D14 / W09）

提供显式 loopback 工程控制服务 `nai-control-api` 和桌面客户端，消费真实 Session 状态，接入独立 EmergencyHandle。W09 增加可注入真实存储 owner 的审批 API；正式 Windows Tauri/受保护 IPC、媒体设备、完整开播预检与审批 GUI 尚未实现。

## 实际支持

| 路由（/api/v1 前缀） | 权限 | 结果 |
|---|---|---|
| GET /health、/runtime/status | runtime.read | ResourceView，当前 Session 快照 |
| GET /sessions/{id} | session.read | 同一作用域的真实快照 |
| POST /sessions/{id}/mute | media.mute | 先撤销媒体信号，再执行 Session 控制状态 |
| POST /sessions/{id}/takeover | session.takeover | 同上，不恢复自动模式 |
| GET /approvals | approval.read | 当前会话/epoch 的待审批元信息，最多 200 条 |
| POST /approvals/{candidate_hash}/approve | approval.write | 哈希/作用域/CAS/期限复检后，原子提交审批、Approved 状态、审计与决策日志 |
| POST /approvals/{candidate_hash}/reject | approval.write | 原子提交 Blocked 状态、审计与 critical cancellation 日志 |

其他操作没有成功占位。OpenAPI 总体标记 partial-loopback-engineering，只有上述操作标记 implemented-loopback。
没有插件或设备时 Session 为 created/standby，不标 Ready；status 的 safe_message 明确说明设备未连接。
CommandAck executed=true 仅表示 Session 控制状态已执行，不能当作真实声卡静音或播放完成 ACK。

## 启动与桌面连接

先由操作员/可信启动器在进程环境中设置 `NAI_CONTROL_TOKEN`：32–256 个字母、数字、下划线或短横线，使用随机临时凭据，不写入仓库或 URL。

```bash
cargo run -p nai-control-api --example local_control --locked --offline
pnpm --filter @nai/desktop dev
```

示例默认仅绑定 `127.0.0.1:4318`；可通过 `NAI_CONTROL_PORT` 指定端口，0 表示动态端口。启动输出地址，不输出 token。
浏览器打开 `http://127.0.0.1:5173`，输入服务地址及同一临时 token，点击连接。
示例真实创建 local-session/local-workspace，设备绑定 local-operator；没有 mock Provider 或模型生成。
token 本轮最长有效 1 小时，不自动续期。`Access::revoke` 可由可信 root 撤销身份；生产配对/凭据 broker 接线仍待实施。
Vite strictPort 保持 origin 明确，CORS 只允许 `http://127.0.0.1:5173`，不放行任意网页。

服务仅显式启动，不作为 Local profile 新增常驻依赖，不改变正式 protected IPC 默认。
只适用于本机工程运行；本轮没有手机、远程 HTTP/TLS 或生产安全部署验收。

## 命令与失败语义

SessionCommand 必须包含 command_id、绑定 device_id、单调 seq、session_id、generation_epoch、expected_state_revision 和 ttl_ms；uint64 用十进制字符串，无精度丢失。
HTTP 请求从生成的 OpenAPI 校验 schema 验证，未知字段拒绝。命令限 4 KiB、TTL ≤30 秒，服务另有 5 秒连接总期限。

- 鉴权、权限、scope、结构与 TTL 检查完成后，静音/接管先调用 EmergencyHandle.mute，不获取业务状态锁。
- 状态更新使用 try_lock，不等待锁；锁忙/CAS/seq/容量失败返回错误，媒体撤权信号保持生效。
- 外层有界命令账本先去重，再验证新命令序号；相同 ID/内容返回原 ACK，改变内容拒绝。最多 256 条、重试保留最多 300 秒，不提前淘汰。
- 桌面命令失败或读取失败后隐藏旧快照，禁用依赖旧状态的操作，需显式刷新；不会自动重发新命令。立即静音不等待普通刷新请求完成。
- UI 断开仅清除客户端 token/状态，不隐式发 Stop；本轮服务未负责生产媒体宿主寿命。
- HTTP 信道失联或连接容量耗尽时不能保证网络静音投递；正式本机独立 P0 仍须保持在运行时/媒体路径，不能以本网页代替设备控制路径。

## 资源与维护边界

服务最多 16 个连接和 16 个任务记录；header 缓冲 8 KiB、最多 32 个 header；请求体 4 KiB，客户端响应上限 64 KiB，客户端 4 秒期限。
并发满时拒绝新连接；关闭服务中断任务并释放监听，不隐式改变 Session。真实 RSS、设备静音 P95、长播与 Windows 安装成本尚未实测。
新增直接依赖复用仓库已锁定的 Hyper/Tokio/bytes/http-body-util；没有引入云主机/Redis/额外常驻服务。

## 审批 backend 接线（W09）

可信 root 打开唯一 `Store` 并保留 `Arc<Mutex<Store>>`，使用
`ApprovalBackend::new(store.clone(), workspace, actor)` 与 `ControlApi::with_approvals` 装配。
不得在审批 backend 为同一数据库再次调用 `Store::open`：重开会恢复/递增 epoch/撤销旧候选，且不允许第二个存储 owner。
mount 拒绝 workspace 不一致；actor 由 root 绑定，HTTP body 不能冒充审批人，存储读写权限每次复检。

`ApprovalCommand` 包含 SessionCommand 全部字段以及 candidate_hash/hash_algorithm。
哈希须为 64 位小写十六进制，与 URL 精确一致，算法为 sha256-canonical-v1。
命令账本与 mute/takeover 共享 seq、256 条容量及 300 秒幂等窗口；重试改变 action 或内容会失败。
每请求新建最长 5 秒的 Operation，写事务还受剩余 TTL 约束；存储锁用 try_lock，忙时立即报 Conflict。
关键日志失败时审批、状态及审计全部回滚，不返回成功 ACK。

存储入库限制同一 workspace/session/epoch 最多 200 个有效待审批候选；超限返回 ResourceExhausted。
列表过滤旧 epoch、过期/已撤销/已批准候选与失效事实/文档；满 200 条标 approvals_limit_reached。
列表只返回哈希/元信息引用/期限/证据计数，U64 为字符串；没有分页或载荷查看接口。
真实候选来源、证据详情页、手机审核与生产 root/媒体接线尚未实现。
`local_control` 示例未配置审批 backend，因此审批请求仍显式返回 CAPABILITY_NOT_READY；测试里的候选仅为测试夹具。

实测报告：[W09 审批闭环](reports/2026-10-09-w09-approval-surface.md)。S02 保持 implemented、O02 保持 pending，不能据 API 测试声称完整 UI/播放验收。

## 审批面（桌面，W38）

W09 之后审批只有 API、没有界面。W38 补上桌面侧的审批面板与可执行断言（[UI 层](ui.md)）。

**可用**：列出待审批候选（候选哈希、状态、证据计数、有效期、200 条上限提示），批准与拒绝，以及**批准前的二次确认**
（首次点击只置位、第二次点击才提交；拒绝是安全方向，单步提交）。

**动作前复核新鲜度**：比较「面板载入时的队列快照」与「动作前的实时快照」——会话换代、候选已不在最新队列、
无有效期、已过期，任一不满足即**不发请求**并给出**具名理由**（以及下一步该做什么）。
把已失效的候选呈现为可点，本身就是错误的反馈。

**文本缺口（未满足的验收）**：设计 §662（O02）与 §425（T-14）要求「审批按钮显示最终文本」。
本栈做不到——`utterances` 表只保存 `text_hash`（`migrations/sqlite/0001-foundation.sql:10`），
审批列表接口也不返回文本。界面如实说明「本载荷不含最终文本、身份为候选哈希」并指向该验收项，**不**把哈希当文本展示。
补齐需存储侧保存可展示文本并同步契约，已登记在 `tests/registry.yaml` O02 notes；**O02 保持 `pending`**。

**命令 ACK 口径不变**：审批/静音/接管的 ACK 只说「控制状态已执行」，界面对应消息必须写明**未取得硬件 ACK**（§30.1「静音 ACK」断言）。

**本轮仍未覆盖**：`local_control` 示例未装配审批 backend（手工点开界面时审批仍返回 `CAPABILITY_NOT_READY`，`tests/ui` 用替身驱动）；
真实候选来源、证据详情页、手机审核与生产 root 接线；不测渲染（不引入 DOM 库），故不构成浏览器或人工 UI 验收。

## 可执行验证

```bash
cargo test -p nai-control-api --offline
pnpm --filter @nai/desktop test
pnpm test:ui
pnpm test:control-smoke
cargo xtask test --suite contracts
cargo xtask check --profile portable
```

`test:ui` 跑 §30.1 指定的 UI 层断言（Node 直接加载 `console.ts`/`control.ts`，无 DOM）；它证明界面行为逻辑，不是浏览器渲染验收，见 [UI 层](ui.md)。

`test:control-smoke` 启动自有示例进程、动态端口和仅内存测试 token，用实际桌面 ControlClient 读取状态并执行 mute/takeover，最后关闭进程。它证明控制闭环，不是浏览器 DOM 或硬件验收。
具体证据见 [本轮报告](reports/2026-10-08-d14-local-control.md)。

## 引擎托管的控制面（engine-assembly）

`apps/engine` 用同一装配启动该控制面：共享会话（撤权 epoch 对装配根立即可见）、真实审批后端、
`0600` 临时 token 与显式 halt 文件（详见 [引擎指南](engine.md)、[ADR 0020](adr/0020-engine-assembly-boundary.md)）。

```bash
cargo test -p nai-engine --offline
pnpm test:engine-smoke
```

`test:engine-smoke` 用真实 `nai-engine` 进程 + 桌面 `ControlClient` 验证 401/status/mute/takeover 与
优雅停机；不产生硬件 ACK，也不代表 protected IPC 或远程认证。
