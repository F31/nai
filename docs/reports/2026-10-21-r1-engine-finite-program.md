# 引擎有限人工辅助节目

日期：2026-10-21。规格：r1-engine-finite-program。决策：[ADR 0141](../adr/0141-engine-finite-assisted-program.md)。

## 实现

- 新增已鉴权节目读写 API，契约源为 control.yaml，已从源生成 Rust 校验和 TS 类型。
- 真实 operator lane 消费 PitchRunner；固定四段、一轮、每段一次生成尝试、总期限 15 分钟；同 epoch 不得重开以清空上限，失败不自动重试。
- next 复用既有知识检索、有效期、四阶段守门、DurableGateway 金额/单位预算、Provider 路由和候选落库。无新增付费旁路。
- 当前候选匹配且有有效审批、本机设备提交才可人工确认前进；离线预览和其他候选不能推进。playback_ack 恒 false，确认不产生已完成句证据。
- 静音/接管/知识切换后旧节目不可继续；无自动归位或五面 Fresh 占位。活跃节目拒绝普通手工派发，需先显式撤权；与 P0 独立。

## 验证

仅执行改动与直接关联范围，不执行全仓 full，不触碰安装版、用户数据库或凭据，不调用真实付费 API。

| 范围 | 实测结果 |
|---|---|
| WSL operator_flow | 25/25；另新增金额预算拒绝用例单独 1/1（并非同次26项运行） |
| 节目状态机 | 3/3：四段顺序与上限、精确人工确认、禁止失败重试、过期和最小切换间隔 |
| Windows 真实 engine 新增节目集成 | 最终 3/3；本地 HTTP fixture，未做设备音频或 OBS 新验收 |
| Windows control-api lib | 51/51 |
| 锁定 Node 契约 schema | 39/39 |
| 契约生成 | cargo xtask schema 完整生成；schema --check 无漂移，contracts TS build 通过 |
| 静态和直接消费者 | Linux/Windows engine + control-api clippy --all-targets -D warnings、fmt、desktop typecheck/build 通过 |
| 注册表 | cargo xtask check --registry 57/57；状态不变 |
| OpenSpec | 已归档为2026-10-21-r1-engine-finite-program；严格主规格44/44、归档183/183通过 |

真实 API 路径验证了启动零调用、同命令幂等、并发下一段拒绝、活跃期普通手工问题拒绝、仅预览不能确认、静音后不可继续、未知事实无模型调用、失败后同epoch重开拒绝。金额用例将请求预估设为2、现有上限设为1，既有 DurableGateway 的 BudgetError::Limit 映射为 RESOURCE_EXHAUSTED，模型请求数为0；初稿测试误期待 BUDGET_EXCEEDED，追到原有映射后修正断言，未修改预算实现。

首轮真实引擎新增 2 项在 Windows 已通过（本地 HTTP fixture）；后补金额用例后最终3项通过。受限环境最初 TCP 返回 10013，获批在原生本机执行后通过。Windows 首次链接有 LNK1104 临时文件锁，同命令重试后通过；Cargo 增量缓存有硬链接失败转复制提示，clippy 检查通过。

WSL 相关 operator_flow 首跑 24/25：事件历史 try_lock 在写入时返回合法短暂 CONFLICT，原轮询器直接要求 200 导致失败。修复只读轮询器，在原 12 秒 deadline 内重试具名 CONFLICT；业务生成请求不重试。

工具入口曾遇自动审批服务额度错误，未执行对应命令；其后审查恢复，已重新按正常审批执行。中间用现有生成器原样片段生成过 HTTP 产物；最终已用锁定工具链完整 `cargo xtask schema` 重新生成覆盖，非手改生成代码。

## 尚未完成

这是引擎 API 生产者，不是整个直播场景交付。桌面面板仍使用原人工辅助流程，尚未消费此 API；下一步接固定四段编辑和引擎状态。进度仅内存，无崩溃恢复或确认审计；四次上限属于生成尝试上限，不是新增可配置节目金额预算/成本预估。完整五面机器校验、插问自动归位、硬件完成 ACK、真人麦克风接管和 OBS 录制验收仍待实现。未新增设备认证，registry 状态不升级。
