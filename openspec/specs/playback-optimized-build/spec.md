# playback-optimized-build Specification

## Purpose
确保V5.6.1常规开发构建使用经真实播放验证的应用CPU优化，并以有界阶段计时区分首次设备启动、像素转换和窗口提交，保留全部媒体安全检查与验收边界。

## Requirements

### Requirement: 应用开发构建优化保留安全检查
系统 SHALL 对nai-workbench-engine的常规dev构建启用opt-level=2并保留debug断言与溢出检查，不以缩小画面或跳过SHA、epoch、取消和期限检查获得性能。
#### Scenario: 重建开发控制程序
- **WHEN** 开发者未传临时优化覆盖参数执行普通cargo build
- **THEN** 应用包使用opt-level=2，既有播放完整性和取消语义不变。

### Requirement: 本地阶段证据与验收分离
系统 SHALL 使用有界聚合记录音频会话启动及像素转换/窗口提交耗时；真实首声包含设备启动，窗口提交不授予硬件呈现或长期通过。
#### Scenario: 首次播放慢而第二次正常
- **WHEN** 首次设备初始化耗时增加
- **THEN** 分阶段耗时与完整首声同时保留，不丢弃首次失败或据第二次通过标记全部验收通过。
