# CosyVoice 生成完成证据内核

状态：C05d1 已实现并定向实测；未部署运行服务。C05d2 HTTP应用EOS/身份缓存与C05d3音画/OBS未完成，C05整体与34项ACC状态不提升。

`scripts/cosyvoice_request_gate.py` 在模型迭代及close退出后锁存终态、PCM样本数和SHA256。正常结束后的ASGI取消清理不改写结果；取消和完成共用短锁，异常/清理失败、空音频、期限和总样本超限均不能生成completed。总样本上限可配置，默认480000。返回结果是副本，不含模型异常正文。该结果仅证明生成线程退出，不证明客户端完整收到音频或硬件播放，也不单独授权缓存。

验证：Windows Python 3.14新增7项CPU测试通过；实际Ubuntu24.04LTS中现有CosyVoice容器运行10项CPU替身隔离回归通过（含断连保留占位、ASGI取消、忙拒绝、模型异常及token状态清理）；项目锁定pnpm/Node执行OpenSpec严格校验通过。没有真实模型调用，没有替换API服务、GUI或用户配置。Windows沙箱首次运行阻止asyncio socketpair，改在批准的本地执行环境运行通过；首次WSL发行版名称错误，枚举实际名称后执行成功。

下一步：先定义并生成实例身份、请求指纹和完成回执HTTP契约，再装配服务与Rust校验；只有HTTP完整交付、当前实例/参考音色/文本及摘要一致的音频才能发布有界缓存。实际缓存命中需独立验证无新模型POST。随后接FlashHead同源样本时钟和OBS。当前Rust仍使用传输EOF形成诊断Seal，不宣称已经修复端到端完整性。
