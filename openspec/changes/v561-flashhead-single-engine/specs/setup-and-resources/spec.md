# 安装、目标机基线与资源

## ADDED Requirements

### Requirement: 目标机单引擎基线

系统 SHALL 以 Windows 11、RTX 5090 Laptop 24GB、32GB RAM 为目标机，默认只加载 FlashHead Lite 一个视觉引擎；LiveAct 与其他第二视觉引擎 SHALL NOT 进入本基线。

#### Scenario: 目标机首次启动
- **WHEN** 用户在目标机启动 V5.6.1
- **THEN** 系统检查 GPU/驱动/VRAM/系统内存/温度，默认选择 FlashHead Lite，并显示是否满足基线

#### Scenario: 请求第二视觉引擎
- **WHEN** 配置或 UI 请求 LiveAct/第二视觉引擎
- **THEN** 系统明确返回不可用并说明已移出主线，不加载第二模型

#### Scenario: 硬件不满足基线
- **WHEN** GPU 显存、系统内存或操作系统不满足最低条件
- **THEN** 系统拒绝进入直播准备，显示具体缺项和可执行建议，不以降级掩盖

### Requirement: 目标机资源基线门

系统 SHALL 在功能开发前执行目标机资源基线：FlashHead Lite 连续 10 分钟、Qwen3-TTS 合成、OBS 捕获；记录 VRAM、系统 RAM、温度、功耗、FPS、首帧、首音与失败。

#### Scenario: 基线通过
- **WHEN** Lite 10 分钟达到 ≥20 FPS、首帧 ≤1.5s、温度 <87°C、无 OOM
- **THEN** Lite 标为可用；Pro 仍为待测，不自动开放

#### Scenario: Pro 未通过
- **WHEN** Pro 在目标机未达到 ≥15–20 FPS 或温度超限
- **THEN** Pro 标为不可用/仅实验，UI 不提供正式选择

#### Scenario: Lite 未通过
- **WHEN** Lite 在目标机无法达到最低实时或发生 OOM/过热
- **THEN** 停止功能开发，记录失败并重新评估模型/驱动/散热，不宣称可交付

### Requirement: 单数据根与无多工作区

系统 SHALL 使用单一数据根、单一 `config.json` 和单一 SQLite 库；SHALL NOT 创建多工作区、多用户、角色或租户分区。

#### Scenario: 首次安装
- **WHEN** 用户首次启动且数据根为空
- **THEN** 系统创建默认数据根、配置和数据库，不要求选择工作区或填写预算

#### Scenario: 路径逃逸
- **WHEN** 配置或导入路径包含 `..`、绝对路径或符号链接逃逸
- **THEN** 系统拒绝该路径并记录原因，不读写数据根之外的文件

### Requirement: 依赖探测与可执行修复

系统 SHALL 分别探测 engine、avatar worker、Qwen3-TTS、FlashHead 权重、ffmpeg、OBS 与音频设备；缺失或未知 SHALL 显示具体原因与修复入口。

#### Scenario: Qwen 未启动
- **WHEN** Qwen3-TTS 端点不可达
- **THEN** 系统显示“Qwen 未就绪”及重试/配置入口，允许进入文字模式但不伪造语音

#### Scenario: OBS 未安装
- **WHEN** 未检测到 OBS
- **THEN** 系统允许本机预览，明确直播输出不可用，并给出 OBS 安装/捕获提示

#### Scenario: 依赖未知
- **WHEN** 无法确认 OBS 捕获或音频信号
- **THEN** 状态保持未知，不显示“已连接/已开播”
