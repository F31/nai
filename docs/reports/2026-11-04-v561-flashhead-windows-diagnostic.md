# FlashHead Windows 单卡兼容性诊断

现有 Windows 环境 Python 3.10.19、PyTorch 2.7.1+cu128，RTX 5090 的 128×128 矩阵运算通过；系统 Toolkit 13.1，PyTorch 自带 CUDA 12.8 运行库。Windows PyTorch 的 NCCL 不可用，但官方源码 `get_device` 只在多卡时初始化 NCCL，不能据此判定单卡不可行。

在 `target-windows/flashhead-win-diagnostic/overlay` 单独安装了 loguru、pyloudnorm、xfuser 0.4.3、distvae、yunchang、win32-setctime、colorama，没有修改原虚拟环境。官方 `generate_video.py --help` 已越过 xfuser 导入并报告使用 PyTorch 注意力回退，当前停在缺少 mediapipe；官方代码即使关闭人脸裁剪仍在入口导入它。mediapipe 0.10.9 Windows 包正在下载到该隔离目录，尚未完成；随后仍需验证其它依赖、GPU 加载和实际推理。

WSL 隔离环境官方 PyTorch 2.7.1/cu128 下载仍在重试。两条路径都没有通过 FlashHead 模型加载或 M0；不根据依赖导入结果推定性能。

后续复核：上一轮的安装会话已无法接续；当前沙箱无法可靠读取隔离目录，不能据此断言 Windows mediapipe 或 WSL PyTorch 是否安装成功。隔离目录只读检查的自动审批因使用额度限制未完成，待权限恢复后重新检查进程、包版本及 GPU 运算。此前的依赖导入结果仍是最近一次可验证证据，M0 状态不变。

本次在恢复权限后核对：Windows 隔离目录可发现 xfuser，但未发现 mediapipe，也未发现相关 Windows 安装进程；WSL FlashHead 虚拟环境导入 torch 失败，且无 uv 安装进程。两套依赖安装都未完成，不能继续宣称正在下载。沙箱对本机 Qwen 端口访问被拒，未据此判断服务是否运行。

用户手动安装后检查：Windows overlay 检测到 mediapipe 0.10.9 和 xfuser 0.4.3；单独导入 mediapipe 时环境报 NumPy 2.2.6 与按 NumPy 1.x 构建模块不兼容，未通过导入。官方 `generate_video.py --help` 的一次运行在导入 SciPy 时报告页面文件过小，未到达帮助输出；同期 OS 报告可用物理内存约 5.67 GiB、可用虚拟内存约 2.21 GiB。未改变系统页面文件或已有环境。WSL flashhead-venv 当时仍不能导入 torch，uv 安装进程仍在运行；尚不可认定 WSL 安装完成。

Windows 修复：在单独的 `target-windows/flashhead-win-diagnostic/numpy126` 安装 NumPy 1.26.4，并在诊断进程优先于现有 overlay 加载；mediapipe 0.10.9 实际导入通过。入口最初仍会通过 Transformers 尝试加载 TensorFlow 并触发 DLL 初始化失败；只对诊断进程设置 `USE_TF=0` 后，官方 `generate_video.py --help` 退出 0。该配置封装于 `scripts/flashhead-windows-diagnostic.ps1`，路径均可传参。FlashAttention 缺失时入口提示使用 PyTorch 回退，推理性能未测。系统虚拟内存不足问题未证明已消除；未修改页面文件，完整模型加载、显存/内存峰值和 M0 尚未验证。

页面文件后续检查：Windows 原为自动管理，当前 C 页面文件约 34458 MiB；E 为固定 NTFS 盘，剩余约 95 GiB。已准备 `scripts/configure-pagefile.ps1`：验证原状态、保存原配置、保留 C 盘系统管理并增加 E 盘 16384–32768 MiB 页面文件，失败时尝试恢复原自动管理；配置需要管理员权限且通常需重启。当前启动管理员进程仍等待系统授权，复查自动管理仍为 True，尚无配置成功或重启验收证据。

页面文件管理员操作首次执行失败：创建 C 盘条目时 Windows 返回“对象或属性已存在”；脚本将自动管理恢复为 True，原 C 页面文件仍存在。脚本已改为复用禁用自动管理后出现的 C 盘条目，并在 E 盘条目已存在时拒绝覆盖；新一轮管理员操作等待系统授权和执行结果，不代表配置已生效。

用户以管理员执行页面文件脚本成功：2026-11-04 21:53 配置记录为 `configured_restart_required`，WMI 显示 C 盘系统管理（0/0），E 盘 16384–32768 MiB；当前运行状态已列出 E 盘 16384 MiB。21:58 查询总提交上限约 78.84 GiB、当前提交约 18.11 GiB；此前 WSL uv 安装进程已不在运行，内存变化不能完全归因于页面文件。Windows FlashHead 入口帮助诊断退出 0。页面文件配置记录要求重启；重启后确认两盘页面文件、提交余量与模型实际加载，不以入口通过代表推理通过。

本轮在未重启情况下确认 E 盘页面文件当前存在（16 GiB），C 盘系统管理；Qwen 8000 端口拒绝连接。Windows 隔离进程首次实际加载已校验 FlashHead Lite、VAE 和 wav2vec2 成功，耗时 21.80 秒，PyTorch 分配 4.00 GiB、保留 4.26 GiB；未生成帧。Wav2Vec2 提示三个参数未从检查点初始化，其中两个卷积参数的检查点使用旧 `weight_g/weight_v` 名称；不将其视为完整有效加载。当前 Transformers 4.39.2、Diffusers 0.30.2、Accelerate 0.28.0 低于官方要求，正在 E 盘独立目录准备官方版本重测。原报告页面文件需重启的验收尚未完成，不代表连续推理或 M0 已通过。

Qwen 启动后联合工程试验：/health 确认为 1.7B，隔离目录安装 transformers 4.57.3、diffusers 0.35.2、accelerate 1.11.0、tokenizers 0.22.1、huggingface-hub 0.36.0。Wav2Vec2 原先缺失的卷积权重在新版本中正确加载，剩余 missing `masked_spec_embed`，unexpected 为 CTC `lm_head`；模型处于 eval，官方调用未传入 mask 索引，静音输入两次输出相同且有限，不能据此宣称质量通过。

Qwen 已驻留时 Lite/VAE/wav2vec2 加载 17.33 秒，PyTorch 分配约 4.00 GiB；先前 Qwen 本地语音转换为 16 kHz 的特征形状 `[1,52,5,12,768]`，有限，至特征输出共 60.44 秒（含加载）。联合驻留期间 nvidia-smi 显存约 9063 MiB，温度 52°C，为瞬时采样而非峰值。

第一次按官方默认编译生成时，在肖像 VAE 编码处因 TritonMissing 失败。第二次仅在隔离测试进程设置 `TORCH_COMPILE_DISABLE=1`，使用已授权 `docs/jjf.jpg` 和已生成的本地 Qwen WAV 成功产生 `target-windows/flashhead-acceptance/jjf-qwen-eager-20261104-222638.mp4`。模型日志三个 chunk 耗时约 1.269、0.679、0.587 秒；ffmpeg 解码确认 512×512、25 fps、52 帧、2.08 秒和 24 kHz 单声道 AAC 音轨；抽帧可见人像。用户口型/听感未验，完整加载、端到端响应及首次延迟不能由 chunk 时间推断；没有实时输出、OBS 或 10 分钟 M0 证据。Qwen 测试后仍健康，未推流。

30 秒持续生成工程检查：以先前 Qwen 1.7B 的 2.08 秒语音重复构造 30 秒诊断音频，使用授权 jjf 肖像、Windows PyTorch eager 回退，不调用新 TTS，不推流。Qwen 服务保持健康。进程总耗时 50.52 秒（包含加载与封装）；官方日志 32 段，第一段 1.404 秒、平均 0.667 秒、最大 1.404 秒。ffmpeg 完整解码输出为 30.00 秒、512×512、25 fps、750 帧，含 24 kHz 单声道 AAC。输出及原始日志分别在 `target-windows/flashhead-acceptance/jjf-repeat30-20261104-225215.mp4` 和同名 `.log`。

该工程检查不是真人使用、真实连续讲品、实时音画或 M0：官方 `generate_video.py` 先将生成帧存入 `generated_list`，结束后统一写 MP4，无法直接证明持续实时输送，也不应直接用作 10 分钟长稳工作流；此轮未采样全程 VRAM/RAM、温度、音画同步和首音。页面文件配置后的重启仍未确认，Triton 与 FlashAttention 未安装。
