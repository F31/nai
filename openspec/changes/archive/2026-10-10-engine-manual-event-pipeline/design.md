## Decisions
编排位置：放在 `apps/engine` 的 pipeline 模块（app 层，可依赖全部下层），不新建核心 crate、不改 kernel。理由：V3.6 §17.2 核心 crate 清单无 pipeline；kernel MUST NOT 含业务策略；AGENTS 要求不预建未到切片的目录。ADR 0022 记录该边界与后续抽取条件（R1 的默认智能体插件落地后再评估是否下沉）。
接缝：`nai-provider-core` 新增 `TextGenerator`（请求含 utterance/上下文/预算，响应含文本与 usage）；确定性桩以显式构造暴露并标注 rehearsal-only，**不得**用于真实集成或认证，也不得替代 API 故障处理。真实 HTTP 适配器与 `UsageGateway` 不变。
链路顺序：manual 事件 → 归一化与去重（有界队列）→ 知识检索（无命中则显式记录）→ 智能体决策 → 守门四阶段（缺守门即拒绝外发）→ 文本生成 → 决策日志 critical/telemetry 落库；任一阶段失败按「先 P0 静音 → 撤权 epoch 递增 → critical 日志 → 有界退出」处理，日志失败不得回退静音。
门控复检：每次进入生成前复检 session、epoch、值守租约与审批对象哈希；陈旧 epoch 或过期审批一律不外发（S01/S03 不变量）。缓存与模板同样经守门。
诚实边界：本轮无 TTS/渲染/设备，故 pipeline 止于文本与决策日志；桩回放失败即视为 R07 判据未达成。文档与报告 MUST 声明无真实 Provider、无设备、无播放 ACK。
## Verification
真实 engine 进程：手工事件经真实 storage 决策日志与真实 kernel 评估产生可校验哈希链；桩回放两次结果一致；陈旧 epoch/守门缺失/注入失败均不外发且落 critical 日志。命令：workspace 测试、check-deps、check --profile portable、check --registry、schema --check、contracts suite、fmt、clippy、前端 typecheck、engine-smoke。
