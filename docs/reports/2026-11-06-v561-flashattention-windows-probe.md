# Windows FlashAttention 2 隔离诊断

从 [发布页](https://huggingface.co/cocktailpeanut/wheels/blob/main/flash_attn-2.7.4.post1%2Bcu128torch2.7.0-cp310-cp310-win_amd64.whl) 下载社区 Windows wheel；首段不完整后按 Content-Range 续传，大小 272147891 字节、SHA-256 `394b7fd1139764a1a6ff1123cd48c9272342707f46d09151413957110e6a7ba5` 与页面一致。哈希仅证明文件一致，不证明发布链可信。wheel 标记 torch 2.7.0，本机为 2.7.1；不宣称 ABI 正式兼容。解包和 CUDA JIT 缓存只在项目 E 盘，未覆盖现有 Python/PyTorch。

CUDA `cuobjdump` 显示二进制含 sm_89 机器码与 PTX，未看到 sm_120 原生机器码。两个限时小算子尝试调用约 50–75 秒未返回后中止；独立 E 盘 CUDA 驱动缓存的第三次完成，首次调用约 89.344 秒，和 PyTorch SDPA 对照 bf16 头维 128、长度 128 的最大绝对误差 0.00390625、平均绝对误差 0.00006849。新进程复用缓存首次调用约 3.125 秒。不能据此确认 JIT 是唯一原因，也不能推广至全部尺寸及动态质量。

768、四步、编译、相同参考和语音离线生成 168 帧：日志 `attention_import` 显示 Flash2 已导入且无更高优先 Sage/Flash3，上游代码中普通注意力分支会选择它；该标记本身不能证明所有算子均走 Flash2。首块约 356.62 秒，后 6 块平均 1.160 秒／0.96 秒音频块，范围 1.140–1.194 秒；此前不同时间默认后端播放试验约 1.278 秒，负载未严格对齐，不能得出精确加速比，当前候选仍不足以证明持续实时。

对照视频 `target-windows/flashhead-acceptance/mouth-768-default-vs-flash274.mp4`：同一肖像、语音及种子，左为此前已认可的默认后端无编译，右为 Flash2 编译候选，均 4 步并统一显示尺寸；用户反馈右侧可接受。编译状态不是唯一变量，因此不能据此严格断言所有数值等价，也不代表真实实时播放和长稳通过。

诊断新增要求 Flash2 的选择守门，缺失或存在更高优先级实现时拒绝；两项选择测试及 OpenSpec 严格校验通过。候选仍无正式来源和目标 ABI/原生 sm_120 验证，不改工作台默认；Qwen 联合合成、OBS、10 分钟 M0 尚未验证。
