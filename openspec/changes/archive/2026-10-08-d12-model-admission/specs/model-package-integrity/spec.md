# Spec Delta

## Purpose
为写实模型提供独立可测试的声明与内容完整性检查，验证模型子图闭包、外部权重和运行时兼容声明，保证缺件、篡改或不兼容不会被当作完整模型，同时保留真实授权、推理正确性和目标机认证的独立边界。

## ADDED Requirements

### Requirement: Model declaration closure
模型检查 SHALL 验证既有 JSON Schema、唯一图 ID、required_graph_ids 引用闭包；文件路径 MUST 为无空段、点段或父目录段的相对路径。重复文件声明 MUST 拒绝。

#### Scenario: Invalid closure
- **WHEN** 模型缺少必需图、图 ID 重复或文件路径有别名/逃逸
- **THEN** 检查失败，不报告内容完整

### Requirement: Content and runtime integrity
检查 SHALL 验证全部声明图与外部权重的 SHA256，拒绝缺失、额外或重复的输入文件，并精确匹配完整运行时矩阵条目；不匹配 MUST 拒绝，不自动回落 CPU。

#### Scenario: Tampered or incompatible model
- **WHEN** 内容哈希不匹配、输入文件集合不一致或指定 CUDA 运行时无精确条目
- **THEN** 返回失败，不生成完整性结果

### Requirement: Bounded cancellable checking
检查 SHALL 限制清单长度、文件数量和累计内容字节，在入口、哈希分块和出口复检 deadline 与 cancellation。

#### Scenario: Interrupted check
- **WHEN** 输入超出限制、检查已取消或期限已到
- **THEN** 返回对应失败，取消/期限失败不产生成功结果

### Requirement: Integrity does not grant certification
成功结果 SHALL 仅表示所给声明和内容的完整性，不授予 G0–G3、插件 Ready 或模型执行权限；没有真实模型、授权和目标机证据时 MUST 保留认证未执行及对应外部 Blocked。

#### Scenario: Test fixture passes
- **WHEN** 测试专用字节及许可引用通过哈希和结构检查
- **THEN** 仅获得完整性摘要，不报告真实模型认证或推理已实现
