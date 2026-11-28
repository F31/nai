## ADDED Requirements

### Requirement: 完成回执先于形象源交付
系统 SHALL 仅在供应商完成回执通过、PCM计数一致且父任务仍有效时生成同Context形象源，收集过程有界且不重试。

#### Scenario: 已收到部分PCM后失败
- **WHEN** 供应商回执失败、任务取消或样本超过限额
- **THEN** 不返回Source，不将HTTP结束视为成功，不保留可复用的部分缓存。

#### Scenario: 同源成功
- **WHEN** 供应商回执成功且全部PCM在限额内
- **THEN** Source绑定当前Context、源ID及实际PCM摘要，仍不授予公共播放权限。
