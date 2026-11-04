# V4.3审视与无旧代码兼容的重构规划

日期：2026-10-22。性质：设计审视/规划交付，不是实现或发布。

## 结论

采纳V4.3作为后续重构目标，通过ADR0147明确与现有安装的区别。产品主线转为真人主导的备播—预演—现场协作—复盘—下场复用；不再把单次提问/候选审批/本机播放当作产品中心。重构可以放弃旧ABI/协议/数据库表/scene JSON/固定四段和旧页面结构，但不得由此推断有权删除现有用户数据、凭据和素材许可。

V4.3是完整目标方案，具备较清楚的产品循环、五维配置、领域对象、DeliveryGate、私有公有隔离、目标ACK、接收端、开放能力和商业验证边界。它仍有示意Schema/路由与待测指标，W01–12只是大工作包，不能直接当已冻结契约/开发时间表，也不能把文档中的“开工”引文当本次用户授权实现。

## 当前实现审视（代码/报告交叉核对）

- Cargo根为单内核/多个共享库/engine/keeper/render-worker和adapter组合，桌面壳独立workspace；无需凭空新建一个仓库或重新发明媒体算法。
- kernel含evaluate/show、platform、review等领域面，目标应收回最小机制，领域逻辑移至policy/agent/live等，而不是继续扩kernel。
- engine/operator.rs约85KB、program.rs约22KB；生产编排在app层，目标应提取领域端口/运行时与单组合根。不能以旧模块名称或部分陈旧文件注释否认已有真实Provider/设备接线。
- desktop/main.tsx约40KB；新界面需按pages/features围绕场次重新组织，不能只是给旧长页换导航。
- 壳provider_db.rs直接持SQLite写连接，管理Provider/产品草稿；引擎另有Store。目标为统一引擎storage owner，壳只保留受控OS交互；配置和Key密文持久化能力保留，主密钥仍受OS vault保护。
- 最新报告证明实际设备时钟、尾帧提交、人工确认持久化及安装挂载；仍没有完整机器AV完成/正式恢复检查点、自动插问归位、真人麦克风和OBS整场证据。新设计必须保留这些证据边界。
- 现有模型列表/主备/音色的易用性原则可保留；旧数据库/场景导出和启动配置链不作为新架构必须兼容的产品模型。

## 需纠正的实施风险

1. 避免将完整架构误解为先建全部crate/微服务；按用户纵向任务创建有真实调用者的模块。
2. 用同一schemas源生成契约，不另建与当前schemas重复可编辑的protocols目录；目标命名保留nai-*。
3. ContentOnly/Assisted先可用，但不能用它通过来代替数字人Avatar场景；媒体与平台准入并行取证。
4. 回执类型决定哪些完成可以推进检查点；不以submitted或人工确认升级机器完成，也不等待不可证明的观众理解才允许其他能力开发。
5. 主持权和交接发送分开，采用Suggestion不直接外发，VAD不授予权限，范围批准须证明对象/目标/TTL边界。
6. 发布影响必须校验revision/graph水位/新会话竞态，未知依赖不是无影响，一般更新不等于全局即时撤销。
7. 复盘/偏好/FAQ采纳到草稿而非权威事实，复制下一场不复制授权/观众身份，计费与真实执行分开。
8. 初始ACC全部待新实现验证；旧测试/本地录制/平台准入/商业复用各自为证，不互相替代。

## 交付物

- ADR0147：目标基线、命名、契约源、注册表与旧资产边界。
- docs/architecture/v43-product-reconstruction.md：领域/进程、数据、状态、UI、完成证据和复用/替换设计。
- docs/v43-reconstruction-plan.md：V43-RF01–12、依赖、M0–M5退出标准、第一卡具体范围及INV-01–40首责。
- docs/acceptance/v43-target-matrix.md：从V4.3第39章逐项提取90项ACC并映射工程卡，只有目标无实际Passed状态。
- docs/design.md、AGENTS.md与OpenSpec上下文指向新重构目标；docs/development-plan.md置顶后续入口，旧进度保留追溯。

## 验证与状态

本轮规划OpenSpec严格校验通过；独立规划脚本核对90项ACC逐项一致且无重复、40项INV完整分责、12张工程卡依赖无环、20条规划文档相对引用有效，AGENTS/OpenSpec设计入口一致。记录`.nai-diagnostics/v43-reconstruction-planning-check.json`。该结果只验证规划，不授予产品或认证通过。README/engineering/R1旧计划亦增加明确的新目标与历史范围提示。未运行Rust/UI/Windows设备/付费/平台/性能/商业测试：本轮无运行代码、契约或数据库变更，不需要全量测试。

RF01及之后工程任务全部未开始。本轮没有创建新内核/空服务、替换安装、改旧库/Key、删除旧工作区、提交Git或联系外部；当前安装仍为原Engineering版。外部规则/厂商链接作为原方案引用，未在本轮独立重新核验，不据此声明现行许可或依赖最新版本。

OpenSpec规划变更已归档为`2026-10-22-v43-product-reconstruction-plan`，新规划规格v43-reconstruction-planning已合入；仅描述规划交付，不承诺90项产品验收已实现。
