# W09 审批 API 子切片实测报告

日期：2026-10-09。环境：Linux/WSL，Rust 1.96.0、Node 22.23.1、pnpm 8.15.0、OpenSpec 1.14.1。
基线：V3.6 / ADR 0015；验收来源 `tests/registry.yaml`。变更：`w09-approval-surface`。

## 已实现

- `nai-storage` 提供 workspace/actor/session/epoch 作用域的待审批列表、单条读取与拒绝；需要 approval.read/approval.write。列表过滤旧 epoch、过期/撤销/已批准及失效事实/文档；元信息不返回载荷。
- `record_candidate` 同会话/epoch 有效待审批候选最多 200，超限拒绝入库。列表 limit 为 1–200，HTTP 满页明确标记 approvals_limit_reached。Approved 状态及旧审批记录不会自动重新入待审批队列。
- approve/reject 在同一事务写审批/状态、审计和 critical 决策哈希链；注入日志写失败时全事务回滚。拒绝推进 Blocked，已批准候选不能拒绝。
- `nai-control-api` 接线 GET /approvals 与 POST approve/reject；使用生成 ApprovalCommand 校验、绑定 device/session/workspace、64 位小写哈希、算法、epoch/revision/seq/TTL、256 条共享账本和 300 秒幂等窗口。
- root 注入当前唯一 owner 的 `Arc<Mutex<Store>>`，backend 不重开数据库；try_lock 不等待存储锁，每次新建最长 5 秒 Operation，写入受剩余命令 TTL 约束。无 backend 显式返回 CAPABILITY_NOT_READY。
- OpenAPI 新增 ResourceItem 的期限/证据计数（U64 十进制字符串），仅三个新增操作标 implemented-loopback；生成产物由 schema 工具生成。

## 定位与修复

1. 修复原接线中的命名空间、分支类型、非 Result 方法使用 `?` 和可变 Store 借用编译错误。
2. 测试原先先 seed 再 `Store::open`：open 的恢复事务正确递增 session epoch 并撤销旧候选，所以列表为空。临时只拷贝主 SQLite 文件遗漏 WAL 恢复更新，导致手工 SQL 与运行中连接不一致。最终保留恢复机制，改为共享同一个存储 owner，并新增重开撤权回归；临时调试代码已移除。
3. backend 启动时创建的单个 5 秒 Operation 会使后续请求永久超时；改为请求级期限，覆盖 backend 存活 5.1 秒后仍能拒绝/幂等重试。
4. 原拒绝路径状态更新、审计、日志分三次提交；现全部同事务，批准也写 critical approval 日志。故障触发器测试证明没有半提交成功。
5. 原列表元信息违反 ResourceItem 的 additionalProperties=false；先扩展 OpenAPI 并生成，再验证真实响应；CommandAck 保持既有字段。

## 实测命令与结果

| 命令 | 结果 |
|---|---|
| `cargo test -p nai-control-api -p nai-storage --offline` | 通过；control-api 15 项、storage 49 项（最终 portable 同样覆盖） |
| `cargo test --workspace --offline` | 通过；262 项 Rust 测试 |
| `cargo xtask check --profile portable` | 通过；13 包依赖边界、registry、fmt、全目标 clippy -D warnings、262 项 workspace 测试 |
| `cargo xtask schema --check` | 通过；27 capabilities、109 protobuf messages，Rust/TS/C++/JSON Schema/OpenAPI 无漂移 |
| `cargo xtask test --suite contracts` | 通过；8 Rust 契约测试、10 schema/API 测试、12 类消息 Rust/TS/C++ 双向互通 |
| `pnpm typecheck` / `pnpm build` | 通过；contracts-ts 与 desktop |
| `pnpm --filter @nai/desktop test` | 通过；5 项既有客户端回归 |
| `pnpm test:control-smoke` | 通过；真实自有控制进程与桌面客户端的 status/mute/takeover 回归，不包含审批 GUI 或硬件 ACK |
| `OPENSPEC_TELEMETRY=0 pnpm exec openspec validate w09-approval-surface --strict --no-interactive` | 通过 |
| `cargo xtask check --registry` | 通过；V3.6 验收状态与证据引用一致 |

contracts 首次运行因整组命令 240 秒总期限在 C++ 编译阶段终止；单独以 420 秒期限重新执行完整 suite 后通过。CMake/gmake 出现约 0.093 秒文件时钟偏差警告，编译、链接与实际跨语言执行均成功。

覆盖：真实 loopback 与临时 Store、批准/拒绝、原 ACK 幂等、改变 action 冲突、已批准拒绝、缺 backend/权限/字段、外会话、错误 epoch/revision/哈希、TTL 过期、锁忙、账本满额、200 候选入库上限、事实更新过滤、恢复撤权、日志失败原子回滚与哈希链校验。

## 验收与缺口

- **S02：implemented**，补齐 API 数据审批证据；真实候选来源、生产 root/播放接线未完成，不提升为完整 tested。
- **O02：pending**，API/存储上限已实测，审批 GUI/高危二次确认/手机审核未实现；不能冒充 UI 验收或外部 Blocked。
- **R07：仍 implemented**，关键审批日志接线已补，但播放 ACK 生产端与完整桩回放仍待后续工作项。
- 本轮没有调用付费 Provider；测试候选只在 tests 中构造。`local_control` 示例未注入审批 backend，仍返回明确不可用。
- Windows 原生 IPC、设备、SDK/GPU、安装认证未执行；本轮 portable 证据不替代真实认证。

## 归档

已归档为 `openspec/changes/archive/2026-10-09-w09-approval-surface/`，6/6 任务完成，新增主规格 `approval-workflow`（3 条需求）。
`OPENSPEC_TELEMETRY=0 pnpm spec:validate`：21 个主规格全部通过；
`OPENSPEC_TELEMETRY=0 pnpm exec openspec validate --archived --no-interactive`：19 项归档全部通过。
