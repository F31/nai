# 输出与录制

## ADDED Requirements

### Requirement: 本机预览窗口

系统 SHALL 提供无边框/可置顶/可隐藏的本机预览窗口，默认 1280×720 @ 25fps；预览 SHALL 消费与公共输出相同的帧流，不另起模型推理。

#### Scenario: 预览
- **WHEN** 进入 ready/live
- **THEN** 预览显示当前形象与安全帧/直播帧，不显示旧 epoch 帧

#### Scenario: 预览关闭
- **WHEN** 用户隐藏预览
- **THEN** 公共输出/OBS 捕获不受影响；不能把关闭预览当作停止直播

### Requirement: OBS 分层状态

系统 SHALL 分别报告 OBS 进程、窗口/场景、采集源、音频路由与实际信号；未知 SHALL 显示未知，不得把进程存在或安装路径存在标为已连接/已开播。

#### Scenario: OBS 进程存在
- **WHEN** 检测到 OBS 但无捕获源/信号
- **THEN** 显示“进程存在、采集未知”，不声称观众可看到画面

#### Scenario: 音频路由未知
- **WHEN** 无法确认 OBS 音频源
- **THEN** 显示音频未知，不声称观众能听到数字人

### Requirement: 本地录制

系统 SHALL 支持 OBS 录制作为首选；SHALL 提供本场录像路径、时长、大小与回放验证；可选内置录制延期，不阻塞主线。

#### Scenario: 录制完成
- **WHEN** 本场结束
- **THEN** 系统记录录像路径并允许回放/校验时长

#### Scenario: 录制失败
- **WHEN** OBS 未开始录像或文件缺失
- **THEN** 系统明确报告失败，不以“已录制”标记

### Requirement: 平台开播分离

系统 SHALL 区分本机输出、OBS 捕获与平台实际开播；SHALL NOT 自动开播平台或伪造平台连接。

#### Scenario: 用户点击开始
- **WHEN** 用户开始本地会话
- **THEN** 只打开本地输出窗口，平台状态保持未开播/未知

#### Scenario: 平台未授权
- **WHEN** 无平台官方授权
- **THEN** 系统只支持手动 OBS 输出，不显示“已连接平台”

### Requirement: 本场报告

系统 SHALL 生成可机读 JSON + 可读报告：时长、首音、首帧、FPS、掉帧、AV 偏差、温度、VRAM/RAM 峰值、错误、动作/片段、引擎/模型/许可版本。

#### Scenario: 正常结束
- **WHEN** 用户停止本场
- **THEN** 生成报告并保存到数据根，不要求用户手动整理

#### Scenario: 异常结束
- **WHEN** worker/engine 崩溃或设备丢失
- **THEN** 报告记录错误与最后状态，不伪造成功
