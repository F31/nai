## MODIFIED Requirements
### Requirement: 同上下文有界音频批次
Root SHALL 只合并同一不可混淆执行上下文中的已验证源，保留各源摘要/精确样本范围并绑定合并摘要，最多两源/144000样本；像素256MiB限额应用于增量输出的Root在途缓冲，不要求为全部未来帧分配像素。
#### Scenario: 不同上下文或取消
- **WHEN** 输入来自不同Context或父Operation失效
- **THEN** 封存/生成 MUST 拒绝或停止，所有尚未播出的依赖帧撤权，不重用剩余帧或恢复旧前缀
#### Scenario: 同一上下文连续输入
- **WHEN** 两段源均合法并已准备
- **THEN** worker SHALL 用单个Render消费精确拼接字节，仅整体末尾补齐生成块，已验证首块可增量交付
