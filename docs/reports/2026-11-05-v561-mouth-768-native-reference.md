# 768 原图直接构图对照

先前 768 的诊断参考构图使用默认 `face_crop_size=512`，输入图先缩小到 512 再用于更大的渲染，不能证明原图原生 768 参考的效果。本轮以同一授权原图直接准备 768×768 参考，与 512 基线使用相同 2.8 构图、语音、种子及输出比例；右侧 768 以 Lanczos 缩至 512，与左侧相同显示尺寸。未添加锐化。对照 `target-windows/flashhead-acceptance/mouth-512-vs-768-native-reference.mp4` 完整解码通过；不同分辨率的随机噪声空间不同，不能保证逐帧表情相同。

无编译 768 的 7 块真实生成完成；排除首块均值 1.329 秒／块，范围 1.315–1.340 秒，每块音频 0.96 秒；峰值 PyTorch allocated 约 6.20 GiB、reserved 约 7.12 GiB。这是离线模型短测，不含生产播放、Qwen 联合负载或 OBS，未能证明持续实时。2026-11-06 用户观看此对照后反馈右侧“明显改善且可接受”，这是该 6.72 秒样片的主观画质认可，不代表完整动态场景或实时播放已验收；生产默认仍须结合实时资源与可靠性决定。

额外候选调研：CodeFormer 项目许可证对商业用途有单独授权要求；GFPGAN 主项目代码标示 Apache-2.0，但其许可证同时列有第三方组件及其他限制，具体代码路径、权重与依赖仍须逐项核实。人脸修复可能改变身份、唇齿和时序，不能因项目开源就直接接入或假定实时。相关上游：[CodeFormer LICENSE](https://github.com/sczhou/CodeFormer/blob/master/LICENSE)、[GFPGAN LICENSE](https://raw.githubusercontent.com/TencentARC/GFPGAN/master/LICENSE)。本轮没有下载或装配新的修复模型。
