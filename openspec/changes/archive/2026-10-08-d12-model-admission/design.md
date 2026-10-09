# Design

## Context
现有 model.schema.json 描述子图、外部权重、预处理与精确运行时版本。仓库未发现 ONNX 文件；V3.5 §9.2–9.3 要求取得完整合法模型后才做特定模型适配。

## Decisions
- 使用 SDK 中独立 model 模块，沿用 schema 源及已有 jsonschema/sha2。无新跨语言消息和手写生成类型。
- 接收宿主提供的不可变内容切片及相对路径，验证全部图/外部权重哈希；避免在 SDK 中新增文件系统加载器。返回清单摘要与文件计数，不返回 Ready。后续宿主必须消费同一份已验证内容，路径解析、symlink 防护和 ONNX external-data 引用解析尚待加载器实现。
- 清单上限 64 KiB，文件数与累计字节由调用方限制；SHA256 每 64 KiB 检查取消/deadline。先检查结构与计数，再进行内容哈希。
- runtime target 使用 schema 的完整矩阵对象精确匹配；拒绝 cpu 回落，不把 certification_ref 的存在视为认证证据。
- required_graph_ids 仅验证声明闭包；真实完整管线与 graph I/O 仍须 ONNX 检查及 G1 对齐。

## Risks / Trade-offs
- 自称完整的清单可能遗漏模型必要阶段 → 不授予认证，真实阶段闭包待模型选择后核验。
- 内容切片要求宿主先准备数据 → 本检查不是磁盘读取预算，宿主须独立实施读取/内存上限。
- 许可引用不证明权利 → G0 保持待证据状态，不添加假认证入口。

## Verification
SDK 测试覆盖闭包/重复 ID、缺文件/哈希错、危险路径、精确矩阵、超限/取消/过期。执行 portable、schema --check、前端及 OpenSpec 校验，记录 D12 部分实现和外部 Blocked。
