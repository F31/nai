# Design

Backend提取共享_infer_block，不复制模型实现。StreamRender一次reset与seed，保持同一deque音频和pipeline motion，AudioInput每满块同步计算；保留一CPU交付块与三槽/逐帧release。有效特征数与补尾分离，尾帧仅EOS裁到ceil(source/960)，不生成额外有效帧。失败先取消job、join交付再释放映射，清除独占流会话。此轮输入为显式已有合法PCM诊断，不称真实TTS或新IPC/GPU组合通过；整段与流式相同768/49/4步/seed，确认像素摘要和恢复后完整帧。
