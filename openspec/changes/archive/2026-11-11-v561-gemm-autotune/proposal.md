## Why
当前编译形状的去噪kernel约69%为BF16 GEMM。音频投影缓存微收益已拒绝。本轮实现针对该热点的显式Inductor GEMM autotune候选，不改变精度或模型结构。
## What Changes
- 只重装去噪transformer块的编译器选项，显式TRITON/ATEN选型，保留VAE和SDPA边界。
- 原240秒维护期限下预热候选，记录新编译证据；同PCM/种子对比基线、候选、回归及全部帧hash。
## Impact
独立诊断候选，默认工作台配置不变；0次API，不安装依赖，不放宽期限。数值/收益不合格则不激活。
