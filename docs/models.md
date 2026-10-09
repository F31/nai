# 模型包完整性检查（D12 前置）

`nai-plugin-sdk::model::check_model_integrity` 接收模型清单 JSON、宿主提供的不可变文件内容切片、精确运行时条目、文件数/累计字节限制、deadline 与 cancellation。

清单唯一源是 `schemas/plugin/model.schema.json`，本轮沿用既有契约，无 schema 或生成类型变更。
检查内容：

- 清单 ≤64 KiB；Schema 拒绝未知字段或缺失必需属性。
- 图 ID 唯一，所有 required_graph_ids 均有声明。
- 相对路径无空段、点段、父目录段或盘符；文件声明及输入路径不重复。
- 全部子图和外部权重必须提供；额外文件拒绝，逐文件验证 SHA256。
- 指定 runtime 对象须精确等于一个完整 runtime_matrix 条目，包括 ORT/crate/EP/精度/驱动/CUDA/cuDNN 和认证引用。没有匹配不回落 CPU。
- 文件数和累计内容字节有上限；入口、遍历、64 KiB 哈希分块与出口复检取消和期限。

成功返回 `ModelIntegrity`（清单 SHA256、文件数与字节数）。这只说明本次提供的内容符合声明；不证明声明列全了真实模型的必要阶段、许可成立或 G0–G3 通过，也不授予 Ready/执行权限。

## 宿主与真实推理边界

宿主须在准备切片时独立执行读取/内存预算、路径和符号链接防护，并让未来推理消费者使用同一份已验证字节。SDK 本接口不打开路径、不解析 ONNX external-data 引用、不加载 ORT。

`avatar-ort`、离线导出、模型特定预处理/后处理、golden 对齐、CUDA 资源实测、取消/OOM/重启认证尚未实现。本仓库未发现 ONNX 包；真实模型授权、完整流程和 Windows GPU 目标机仍需证据。参见 [本轮报告](reports/2026-10-08-d12-model-admission.md)。

```bash
cargo test -p nai-plugin-sdk --offline
cargo xtask check --profile portable
```

`tests/model.rs` 中图与权重为明确测试字节，不是可运行模型或授权凭据。
