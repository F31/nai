# 直播会话

## ADDED Requirements

### Requirement: 会话状态机

系统 SHALL 提供 `idle/preparing/ready/live/paused/stopped/faulted` 状态，并保证每次转换有明确入口/出口、超时与错误状态；停止/故障 SHALL 优先于其他转换。

#### Scenario: 开始
- **WHEN** 用户点击开始且依赖满足
- **THEN** 依次检查依赖、加载模型、预热、进入 ready，再由用户确认进入 live

#### Scenario: 依赖缺失
- **WHEN** Qwen/模型/OBS/许可任一缺失
- **THEN** 会话停在 preparing 或 faulted，显示具体缺项，不进入 live

#### Scenario: 停止
- **WHEN** 用户点击停止
- **THEN** 立即进入 stopped；取消 LLM/TTS/worker，清队列，静音，安全帧

### Requirement: 单操作员控制

系统 SHALL 由本机单操作员控制，不要求登录、角色、审批或逐句确认；SHALL 提供开始、暂停、静音、接管、停止与打断。

#### Scenario: 接管
- **WHEN** 用户点击接管
- **THEN** 停止 AI 语音/动作，切真人麦克风，保留转写与上下文；恢复 AI 需显式操作

#### Scenario: 静音
- **WHEN** 用户点击静音
- **THEN** 只停数字人音频，画面切待机；真人麦克风不受影响

#### Scenario: 停止优先
- **WHEN** 停止与生成/动作/队列同时发生
- **THEN** 停止先执行，不等待当前句子或动作结束

### Requirement: 连续讲解

系统 SHALL 支持主题/资料/脚本驱动的连续讲解；段落队列有界；支持暂停、继续、跳过、停止；SHALL 在资料不足时明确说明，不把猜测标为事实。

#### Scenario: 主题讲解
- **WHEN** 用户输入主题并开始讲解
- **THEN** 系统生成/读取段落，按队列合成与播放，播完自动下一段

#### Scenario: 资料不足
- **WHEN** 当前资料无法支持某陈述
- **THEN** 系统说明不确定/跳过，不编造已验证事实

### Requirement: 问答与句边界归位

系统 SHALL 支持文字/事件问题触发 AI 回答，回答按句合成/播放，结束后回到讲解检查点；停止/接管/新问题 SHALL 取消旧回答。

#### Scenario: 回答后归位
- **WHEN** 一个问答完成
- **THEN** 系统回到讲解进度，不重复已播段落

#### Scenario: 新问题打断
- **WHEN** 高优先级问题到达
- **THEN** 在句边界暂停当前讲解、处理问题、再归位；停止/接管立即取消

### Requirement: 不自动开播平台

系统 SHALL 只输出本机窗口/OBS；SHALL NOT 自动连接或开播抖音/快手/视频号等平台；OBS 进程存在不等于已捕获/已开播。

#### Scenario: OBS 运行
- **WHEN** 检测到 OBS 进程
- **THEN** 仍分别显示窗口捕获、音轨、信号、平台开播状态；未知保持未知

#### Scenario: 用户开始直播
- **WHEN** 用户点击开始
- **THEN** 只开始本地会话与输出，不自动调用平台开播
