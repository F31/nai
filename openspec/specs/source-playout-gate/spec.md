# source-playout-gate Specification

## Purpose
为同一次TTS返回的PCM与GPU帧建立有界交付和覆盖水位，区分源释放与设备实际播放，遇真实欠载或取消时终止任务。

## Requirements

### Requirement: 同源交付
Rust SHALL 把同一次TTS返回的PCM按相同顺序交媒体sink和GPU，验证EOS后才允许尾部提交，sink失败进入统一取消清退。

#### Scenario: 单次真实合成
- **WHEN** 一次供应商返回分块PCM及验证EOS
- **THEN** 媒体sink释放的总样本与SHA等于供应商，且GPU读取同一源；不得额外调用TTS。

### Requirement: 有界画面覆盖水位
PCM储备 SHALL 有配置上限及预填充，只释放连续已验证帧覆盖的源音频；真实需求不足必须终止，普通轮询等待不算欠载。

#### Scenario: GPU落后
- **WHEN** 已开始的设备需求超出当前画面覆盖
- **THEN** 返回欠载并取消任务，不补静音冒充播放，不借墙钟推进。

#### Scenario: 短句及尾帧
- **WHEN** EOS已验证且全部画面覆盖短句
- **THEN** 可以低于预填充启动，最后一帧只覆盖真实源样本。
