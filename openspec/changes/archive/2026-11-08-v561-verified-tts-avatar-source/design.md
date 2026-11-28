# Design

Context拥有唯一Operation。收集器接受规范24kHz单声道PCM，限制144000样本；每块检查取消、偶数字节和容量。现有CosyVoice adapter负责请求/实例/样本/摘要完成回执。只有其成功并且计数一致且Context仍有效时返回Source。失败丢弃缓冲，零重试。完整句签封是当前worker完整源协议的显式边界，不伪装成在线GPU块推理。
