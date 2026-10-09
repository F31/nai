# D12 前置：模型包完整性检查实测报告

- 日期：2026-10-08；环境：Linux/WSL、项目锁定 Rust/Node/pnpm。
- 基线：V3.5 §9.2–9.3；本轮 OpenSpec：`d12-model-admission`。
- 结论：声明与所给内容完整性检查已实现/实测；D12 写实推理管线尚未完成。

## 实现范围

`nai-plugin-sdk::model::check_model_integrity`：沿用模型 JSON Schema，验证唯一图 ID、必需图引用闭包、规范相对路径、声明文件集合、全部图/外部权重 SHA256、精确完整 runtime_matrix 匹配、文件数与字节预算、取消/deadline。
哈希以 64 KiB 分块检查取消和期限。无新增依赖、进程、schema 或生成类型变更。
输入是调用方提供的不可变内容切片；返回清单哈希、文件数与字节数，仅表示本次内容匹配声明。

## 实测

| 命令 | 结果 |
|---|---|
| `cargo test -p nai-plugin-sdk --offline` | Passed：14 项既有单测 + 6 项新增模型测试 |
| `cargo clippy -p nai-plugin-sdk --all-targets --offline -- -D warnings` | Passed |
| `cargo xtask check --profile portable` | Passed：11 包依赖边界、fmt、workspace Clippy、205 项 Rust 测试 |
| `cargo xtask schema --check` | Passed：三语言/JSON Schema/OpenAPI 生成无漂移 |
| `pnpm typecheck` / `pnpm build` | Passed |
| OpenSpec strict validate（实施前） | Passed |
| 实施后 strict validate / archive / spec:validate / archived validate | Passed：14 个主规格、12 项归档 |

6 项新增测试覆盖：正确摘要；缺图/重复图/缺许可或预处理字段/未知字段；路径别名/逃逸/重复声明；缺件/额外文件/重复内容/篡改权重；运行时所有字段精确匹配且不 CPU 回落；清单/文件数/字节超限、取消与过期。
测试图及权重是明确的测试字节，不是 ONNX 模型；没有调用真实推理或付费 Provider。

## 真实状态

- **已实现/实测**：声明与内容完整性前置检查。
- **未开始**：avatar-ort、tools/model-export、模型特定音频预处理/编码器/生成/后处理、宿主模型加载器与 ONNX external-data 引用核对。
- **Blocked**：仓库未发现 ONNX 文件，完整合法模型包及授权证据尚缺；本轮没有 Windows GPU 目标机证据。
- **未执行**：G0–G3、Windows/CUDA/直播工具整链路、T/E/U 认证。许可/认证引用存在不等于证据通过。
- 本轮不重跑完整 C++ 互通；生成无漂移不冒充互通认证。

调用方仍须实施文件读取预算/路径/symlink 防护，并让未来执行器消费同一份已验证内容。必需图列表是模型作者声明，不能代替真实完整阶段清单或 G1 golden 对齐。

本轮前置变更已归档至 `openspec/changes/archive/2026-10-08-d12-model-admission/`，5/5 任务完成，4 条需求合并为 `model-package-integrity` 主规格；归档的是本轮完整性检查范围，D12 整体仍未完成。
