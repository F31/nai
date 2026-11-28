# 验收与发布

## ADDED Requirements

### Requirement: V5.6.1 ACC 注册表

系统 SHALL 建立约 30 项 V5.6.1 ACC，覆盖 SETUP/TTS/HEAD/PIPE/LIVE/OUT/REL/RES/UX；每项 SHALL 包含步骤、预期、证据类型、环境与结果；SHALL NOT 继承 164 项旧 ACC 或把设计条目当通过。

#### Scenario: 新增验收项
- **WHEN** 增加能力/故障/性能项
- **THEN** 先在 ACC 注册表登记 ID、步骤、证据与适用阶段，再实现/执行

#### Scenario: 旧项
- **WHEN** 旧 V4.8/V5.6 ACC 与本变更相关
- **THEN** 只能作为参考；未映射/未重测不授予通过

### Requirement: 分层阶段门

验收 SHALL 按 M0 目标机基线、M1 30 分钟、M2 2 小时、M3 4/8 小时递进；SHALL NOT 在 M0/M1 未通过时宣称 M3/8 小时可用。

#### Scenario: M0 未通过
- **WHEN** FlashHead Lite 10 分钟未达到 FPS/首帧/温度/内存阈值
- **THEN** 阻断后续发布声明，先修复/重测

#### Scenario: M1 通过
- **WHEN** 30 分钟真实 Qwen+FlashHead+OBS 通过
- **THEN** 才允许进入 M2 长稳；不跳过 2 小时直接 8 小时

### Requirement: 真实环境证据

每项通过 SHALL 关联真实环境证据：设备/驱动/模型/权重哈希/参数/样本/负载/阈值/原始指标/录像或日志；SHALL 区分实现检查、模拟、Blocked 与真实认证。

#### Scenario: 模拟测试
- **WHEN** 使用 fake Qwen/fake worker
- **THEN** 标为 `simulated`，不能替代真实 GPU/Qwen/OBS 认证

#### Scenario: 未测
- **WHEN** 缺少目标机/真实服务/真实用户
- **THEN** 标为 `pending`/`blocked`，不标 `passed`

### Requirement: 发布清单

发布候选 SHALL 生成 Release Manifest：版本、commit、模型/权重哈希、许可结论、ACC 结果、已知限制、适用设备、默认档位（Lite/Pro）、未实现/延期项；SHALL NOT 只凭 CI 绿或文档齐全发布。

#### Scenario: 许可未确认
- **WHEN** FlashHead 权重/VAE 或 LiveAct 许可未确认
- **THEN** 阻断相应发布/商用声明；清单显示限制

#### Scenario: 性能未认证
- **WHEN** Pro 未在目标机验证
- **THEN** 清单只声明 Lite；Pro 标为不可用/实验

### Requirement: 文档与看板同步

实现 SHALL 同步更新 `docs/development-plan.md`、`docs/reports`、README、设计索引与验收看板；SHALL 归档 V4.8/V5.6 旧执行状态，不拼接旧结论。

#### Scenario: 完成阶段
- **WHEN** M1/M2/M3 完成
- **THEN** 写入真实报告与 ACC 状态；未完成项保持未勾选

#### Scenario: 设计变更
- **WHEN** 范围/模型/默认值变化
- **THEN** 更新本变更 specs/design/tasks，并记录 ADR/报告；不只在聊天/临时文档里改

### Requirement: 验收模型与当前基线一致

V5.6.1 的验收命令 SHALL 与现行 ADR 的首轮 Qwen 模型一致；更新模型描述 SHALL NOT 自动更改验收状态或视为测试通过。

#### Scenario: 首轮采用已部署 1.7B
- **WHEN** 检查 TTS 短句验收步骤
- **THEN** 步骤指定当前 1.7B，并保持原有 pending 或实际证据状态
