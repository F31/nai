# 动态底片候选初筛

选择 FeatherTalk 为唯一深入核查候选：https://github.com/anliyuan/FeatherTalk ，固定提交 `7a8bb0439eb5da5e10601168df329a7052b7edf3`。源码对象在 E 盘 `target-windows/avatar-region-research/FeatherTalk`；只读取源码及说明，未训练、未加载权重或运行第三方模型。

官方描述为个性化局部面部模型，需要 3–5 分钟 25 FPS 单人口播视频，每人物训练视觉权重。现有授权视频约 186 秒、30 FPS，需要独立转换并检查音频及姿态。视觉输入仅 144×144，未证明可达到用户认可清晰度；离线推理和 C++/MNN 接口也不代表完整在线流式已实现。

代码 LICENSE 为 Apache-2.0，FeatherHuBERT 权重在官方说明中提供来源与哈希；但默认预处理依赖 `scrfd_2.5g_kps.onnx` 和 `checkpoint_epoch_335.pth.tar`，完整来源及许可待核查，代码许可不能代替权重许可。下一步核查或替换这条检测／关键点链，不加载未核清权重，再进行独立人物训练和完整样片验收。

本候选尚未准入；OpenSpec 第 2 项继续未完成，未恢复旧 MuseTalk／LivePortrait，不改变 V5.6.1 和 34 ACC 状态。
