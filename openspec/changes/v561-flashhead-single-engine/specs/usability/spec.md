# 易用性

## ADDED Requirements

### Requirement: 一键准备与默认引擎

系统 SHALL 提供一键准备：检查依赖、加载 FlashHead Lite、预热、进入待机；普通用户 SHALL NOT 需要选择引擎/模型/线程/显存参数。

#### Scenario: 首次使用
- **WHEN** 用户首次打开且数据根为空
- **THEN** 系统引导选择形象/试听/预览，默认 FlashHead Lite，不要求理解引擎概念

#### Scenario: 依赖缺失
- **WHEN** Qwen/模型/OBS 缺失
- **THEN** 页面显示具体原因和修复入口，不使用技术错误码作为唯一提示

### Requirement: 简洁主界面

系统 SHALL 在主界面只保留预览、开始/停止、静音、接管、输入、动作；引擎/画质/资源/许可/日志 SHALL 放在状态页或高级设置。

#### Scenario: 直播中
- **WHEN** 会话处于 live
- **THEN** 停止/静音/接管始终可见且可键盘到达；不显示引擎切换开关

#### Scenario: 高级设置
- **WHEN** 用户打开设置
- **THEN** 可配置 LLM/Qwen/输出/过滤/热键/数据根/许可，不强制每次填写

### Requirement: 状态可理解

系统 SHALL 用可理解状态表达准备中/待机/直播中/降级/已停止，并说明降级原因；SHALL NOT 用绿色“已连接”覆盖未知/部分失败。

#### Scenario: 降级
- **WHEN** 从 Pro 降 Lite 或从视频降静态
- **THEN** UI 明确显示“已降级/原因/当前能力”，不静默

#### Scenario: 未知
- **WHEN** OBS 信号/平台状态未知
- **THEN** 显示未知，不显示已连接/已开播

### Requirement: 热键与可达性

系统 SHALL 提供全局停止/静音/接管热键，不依赖 UI 焦点；关键操作 SHALL 可通过键盘完成。

#### Scenario: UI 卡顿
- **WHEN** 主界面无响应
- **THEN** 全局停止仍可用；若不可用则记为 P0 缺陷

### Requirement: 许可与首次引导

系统 SHALL 在首次导入形象/声音时要求确认使用权，并记录时间/来源；未确认 SHALL 拒绝开始直播；SHALL 显示模型/权重许可状态。

#### Scenario: 未确认许可
- **WHEN** 用户未确认肖像/声音权利
- **THEN** 开始按钮禁用/提示，不静默使用

#### Scenario: 许可未知
- **WHEN** 模型权重许可未确认
- **THEN** 显示风险徽标，限制发布/商用开关，不宣称可用
