# PyTorch profiler 首次有效采集

修正缓存初始化为绝对路径后，独立诊断正常运行，并成功导出第二块 CPU/CUDA 时间线。768 对照及编译路径尚未采集。本次 512／49 帧／四步／SageAttention 2.2，固定素材和种子，模型与 VAE 不编译，只运行 2 块，已有语音，无播放、Qwen 或 OBS 并发。新增 `--profile-trace` 仅对第二块采样，采样耗时不作为实时基准。

时间线 `target-windows/flashhead-acceptance/torchprofile512-20261106.trace.json` 有 11716 个 kernel、14406 个 CUDA runtime 事件。GPU kernel 时长累计约 637.7 ms，gpu_memcpy 累计约 4.15 ms；主要单类核包括 cuDNN bf16 卷积约 201.3 ms、两类 CUTLASS 矩阵乘约 172.2 和 58.5 ms。CPU 算子嵌套计时不能直接相加，kernel 时长累计也不等于无重叠 GPU 墙钟；不能据此单独认定 CPU 或 GPU 已是唯一瓶颈。

第二块采样期间约 1.889 秒，其中音频特征 0.067 秒，渲染 1.822 秒。采样本身增加开销，且没有启用编译，不能直接与之前编译版比较。已确认能取得 GPU 核事件，不再依赖失败的 Nsight 启动。下一步同条件采集 768，并与编译路径及无采样短测配对，依据真正热点选择优化；不再凭文件传输大小推断复制瓶颈。
