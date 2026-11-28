# FlashHead 形象渲染

## ADDED Requirements

### Requirement: 可验证的人脸构图预处理

形象预处理 SHALL 允许显式启用人脸居中构图并配置留白比例；SHALL 保留原始素材，拒绝无人脸、多张合格人脸或无效裁剪区域，不得静默退回原图。诊断结果 SHALL 标识构图参数，不得将离线质量样片视为实时性能或生产许可证明。

#### Scenario: 合法单人素材
- **WHEN** 用户启用人脸构图且素材中恰有一张满足阈值的人脸
- **THEN** 生成独立构图用于模型输入，保留原图并记录参数

#### Scenario: 无法确定单一人脸
- **WHEN** 没有检测到或检测到多张满足阈值的人脸
- **THEN** 明确失败，不生成误导性的回退画面

### Requirement: 单引擎 FlashHead

系统 SHALL 只加载一个视觉引擎，默认 FlashHead Lite；SHALL NOT 在同一会话加载 LiveAct、MuseTalk、LivePortrait 或第二个视觉引擎。

#### Scenario: 默认启动
- **WHEN** 用户开始准备形象
- **THEN** 系统加载 FlashHead Lite 并显示模型/权重/许可状态

#### Scenario: 请求 LiveAct
- **WHEN** 配置或 UI 请求 LiveAct
- **THEN** 系统返回不可用并说明已移出主线，不加载 18B 模型

### Requirement: 单一当前形象

系统 SHALL 支持一个当前形象：内置示例或用户导入的合法肖像；SHALL 校验格式、尺寸、人脸有效性、哈希与许可确认；SHALL NOT 同时驻留多个形象版本。

#### Scenario: 导入合法肖像
- **WHEN** 用户选择支持格式且勾选使用权
- **THEN** 系统保存源文件、哈希、许可记录，并允许预览/说话

#### Scenario: 肖像无效
- **WHEN** 文件损坏、无人脸、尺寸越界或许可未确认
- **THEN** 系统拒绝导入并显示具体原因

#### Scenario: 替换形象
- **WHEN** 用户选择另一形象
- **THEN** 系统排空当前任务、卸载/替换模型，不保留旧形象帧

### Requirement: 说话与待机

系统 SHALL 在说话时使用 FlashHead 由音频驱动口型；在待机/倾听时输出稳定待机帧；停止/失联/无音频时 SHALL 输出安全帧，不得继续旧嘴型配新音频。

#### Scenario: 连续说话
- **WHEN** 音频持续输入
- **THEN** 系统输出连续口型帧，帧带 epoch/seq/pts，音画按音频时钟对齐

#### Scenario: 无音频
- **WHEN** 当前没有语音或处于待机
- **THEN** 输出待机/安全帧，不随机张嘴或重复旧说话帧

#### Scenario: 停止
- **WHEN** 用户停止或静音数字人
- **THEN** 输出安全帧；旧帧/旧音频不得续播

### Requirement: 质量与能力边界

系统 SHALL 在目标机验证身份保持、口型、牙齿、侧脸、遮挡、长句、数字/型号；对超出认证范围的输入 SHALL 拒绝或切静默展示，不承诺全身、手势、任意动作。

#### Scenario: 侧脸/遮挡超范围
- **WHEN** 输入超出已认证角度或手遮嘴
- **THEN** 系统拒绝口型或切静默展示，并记录原因

#### Scenario: 长句/数字
- **WHEN** 合成包含长句、数字、型号
- **THEN** 系统记录口型/发音质量；明显失败不标记通过

### Requirement: FlashHead Pro 可选

系统 SHALL 仅在目标机实测达到 ≥15–20 FPS 且温度可控时开放 FlashHead Pro；未通过时 SHALL 置灰/隐藏，不作为默认。

#### Scenario: Pro 实测通过
- **WHEN** Pro 在目标机 10 分钟达到阈值且无 OOM/过热
- **THEN** 允许作为可选画质档，默认仍为 Lite

#### Scenario: Pro 未通过
- **WHEN** Pro 低于阈值或触发过热
- **THEN** Pro 不可用，UI 说明原因；不回退到未认证参数

### Requirement: 模型加载与资源上报

系统 SHALL 上报模型加载、预热、首帧、FPS、VRAM、系统 RAM、温度；模型 SHALL 常驻当前会话，换形象/停止时按明确生命周期卸载。

#### Scenario: 加载失败
- **WHEN** 权重缺失/哈希不符/OOM
- **THEN** 系统显示错误并保持安全帧，不进入直播

#### Scenario: 预热
- **WHEN** 模型加载完成
- **THEN** 空转 2–3 帧并上报首帧/资源，再允许开始
