## Why
接管512/3实现发现Rust/Python已支持sample_steps，但avatar-worker JSON Schema未声明；additionalProperties=false会拒绝合法配置，生成TS类型也缺字段。
## What Changes
- 对齐显式2/3/4步配置契约及生成类型，不改变既有四步默认或三步时间表。
- 定向验证Schema/Rust/Python，记录下一性能主线的源码和时间线审视。
## Impact
仅修复配置契约一致性，无模型API或用户数据改动，不授质量/实时通过。
