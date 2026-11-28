# played-pcm-source Specification

## Purpose
TBD - created by archiving change v561-played-pcm-source. Update Purpose after archive.

## Requirements

### Requirement: 设备同源PCM封存
Rust owner SHALL 只复制实际提交设备的同一份PCM，按样本及字节上限约束临时源，并在供应商完成验证及设备完整播放后核对总摘要和样本数再封存。

#### Scenario: 完整真实源
- **WHEN** 设备Completed且累计样本/摘要与临时源一致
- **THEN** 源描述 MUST 绑定实际设备epoch并可交给worker独立复检

#### Scenario: 不完整或取消
- **WHEN** 出现摘要不符、超限、未完成或父Operation取消
- **THEN** 源 MUST 拒绝返回可渲染描述，临时文件在owner释放时删除

### Requirement: 不扩大验收结论
诊断 SHALL 区分真实同源输入验证与公共同步播放。

#### Scenario: 先声音后生成
- **WHEN** 实测声音已播放结束才启动形象渲染
- **THEN** 报告 MUST 明确不表示同步窗口、持续实时或M0通过
