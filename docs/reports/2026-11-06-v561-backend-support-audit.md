# Windows 计算后端支持核查

实际导入 PyTorch 2.7.1+cu128，CUDA 12.8，cuDNN 9.7.1；GPU capability 为 12.0，包内 arch_list 包含 sm_120。NVIDIA cuDNN 9.7.1 支持矩阵明确列出 CUDA 12.8／Blackwell。因此不能以 profiler 核名称中的 sm80 推断硬件不支持或强行架构回退，也没有证据证明驱动损坏。

在隔离进程尝试 `preferred_blas_library('cublaslt')`，PyTorch 明确警告此功能 Windows 不支持；查询仍返回 Cublas，bf16 小矩阵运算有限。该测试仅证明选项被忽略及基础运算可执行，没有测得可比较候选，不得称 cuBLASLt 优化成功。该进程结束，不改变应用默认。

下一步若继续做原生后端比较，应先验证新环境确实改变实际支持能力及内核选择，再用真实 FFN 输入比较；不为相同被忽略的开关反复重测。当前版本的 Windows BLAS 选择路线停止，考虑受控 Linux 同硬件比较或另立规格的区域生成；Linux 比较不替代 Windows 验收，也不能预先声称更快。完整生产模式不变。

一手依据：https://docs.nvidia.com/deeplearning/cudnn/backend/v9.7.1/reference/support-matrix.html
