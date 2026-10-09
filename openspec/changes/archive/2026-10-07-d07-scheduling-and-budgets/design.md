# 设计

kernel/scheduler 提供 session 作用域的 ingress/candidates/utterances，完全不依赖 Provider 或存储。
普通事件 FIFO；关键状态按受控来源+种类独立槽位 coalesce 最新状态，时间/序号不明确时不声称严格排序。
事件去重记录和队列容量有界，不提前丢弃重试 TTL 内的 dedup；溢出显式统计。礼物只在同来源/观众/combo 的可解释语义下合并，unknown 不加总。

候选使用单调 deadline，session epoch 显式同步失效；主题合并不延长旧 TTL。选择优先级 P1–P5，
低优先等待达到配置阈值后提升；观众冷却和滚动选择窗口占比用于公平。P0 复用 D06 EmergencyHandle。
speech debt 为已选中未结清的估计 speech_ms，必须同时通过容量/debt/播放期限；消费失败不会免费清账。
Provider admission 由可信 root 回调，仅对候选选择后的任务调用。回调预留持久账本后，返回的 Selected 才可开始付费生成。

新增 nai-budget-core（shared，只依赖 contracts）负责精确 u64 金额与多维 counters，普通预算与并发分离。
一个 workspace 一个共享 Book，所有 session/provider ceiling 显式登记；单 owner 原子计算全局/session/provider。
Protobuf 新增 BudgetCharge/UsageLedgerEntry/BillingState，配置增量可选且保持 schema_version=1 兼容。
BudgetCharge 的单位由 profile 映射为 requests/tokens/characters/speech_ms/tool_steps 等整数；GPU/Provider 并发属于单独 concurrent 计数。
预留可取消但只有已证实未派发才释放；派发后取消保留费用及并发直至实际工作结束。
finish(None) 证明任务结束，释放并发但保留估算；对账 actual 替换费用。费用超估算仍保存（overdrawn），后续准入被拒绝。
溢出/未知单位拒绝并保持已有记账，不使用 f64。单币种、显式 price_revision；不隐式汇率转换。

storage 使用既有 usage_ledger estimate/actual JSON 与 billing_state 列持久化 Protobuf 语义，
JSON 中 u64 字符串保存，不手改生成文件。预算修改在 SQLite owner 事务内重建 Book→变更→upsert→审计→提交；
max_entries 上限防止无限读入。重启保留 Reserved/Dispatched，不自动释放/重发；owner root 控制 limits，Provider/模型不能自报限额。
派发使用事务内 DispatchClaim：只有本次成功由 Reserved 转成 Dispatched 的 acquired=true 能启动 Provider；
ACK 丢失重试返回 acquired=false，通用 update_usage 不接受 Dispatch，避免仅凭“已派发”记录重复调用。
批量恢复先验证完整集合再计算总量，避免逐行全量扫描；费用未知仍保留估算，请求尝试计数不能被 token-only actual 抹掉。
已有表无列变更，不新增迁移；旧未解析格式保持明确拒绝而非当零账单。

验证 scheduler suite 包含 kernel/budget-core/storage；输入满载、礼物语义、公平/冷却、过期、债务、拒绝准入、
跨 session 共享限额、取消/未知/超估算/幂等对账、并发占用、重启持久预留、溢出原子性。
portable 全量、契约生成漂移、三语言互通/TS 构建、Windows 可用交叉检查与 OpenSpec strict。
真实 Provider usage/bill 对账属于 D08，实际音轨债务反馈属于 D11，Windows 原生环境仍 Blocked。
