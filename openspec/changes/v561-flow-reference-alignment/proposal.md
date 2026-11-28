## Why
首块元数据实测参考87 token、基础hop25、padding13、lookahead3，实际首次解码等待41 token。固定Flow chunk25不允许盲改更小hop；评估仅Flow参考上下文对齐，保留完整LLM提示/声纹，不更换模型。
## What Changes
- 显式可配置Flow参考对齐候选，基础hop/特征比例由当前模型读取，最小参考长度显式准入。
- 缓存不污染、LLM/embedding不变、四个Flow条件字段同步处理，质量/取消/恢复门独立验证。
## Impact
本轮仅规格与采证；候选未实现，不激活默认、不修改用户素材或原服务。
