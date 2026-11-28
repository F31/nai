# 参考音色条件缓存（2026-11-10）

状态：单项内存缓存实现、20项CPU检查、空闲维护激活及真实miss/hit验证通过。原有单TTS/FP32/768四步不变，不授稳定低延迟、持续实时或长稳；34ACC仍pending，M0仍No-Go。

## 证据与策略

此前阶段记录sequence7首PCM14206ms，frontend11644ms，其中_extract_speech_token10561ms；正常记录该提取约673–1225ms。这里只能定位Python wall时间落在哪一阶段，不足以唯一归因GPU/CPU/分页。相同参考音频每句重复处理是可消除工作，因此缓存条件而不是缓存回复正文。

新增`scripts/cosyvoice_condition_cache.py`和严格入口补丁。每个frontend实例仅存1项，参考数据及条件各最多16MiB，以完整提示词、采样率、Tensor dtype/shape和内容区分；不按文件路径命中。每次命中独立编码当前回复文本、克隆条件张量，原生非空spk_id旁路。内容变化替换、超限旁路、失败不留半成品。仅内存、不新增API，不持久化参考特征或打印缓存key；日志只计数和字节数。缓存不授播放或素材使用权限，不改变请求门控。

## 检查与部署

- 12项缓存/严格补丁检查通过，含不同文本、修改隔离、内容/提示词/采样率失效、容量/禁用/原生speaker旁路、失败、重复安装、实例隔离及观察器组合；相邻8项阶段观察测试通过。未运行全仓测试。
- E盘`target-windows/v561-voice-condition-cache/`留存原服务/构建入口、Dockerfile及候选。部署比对来源未变化，健康空闲后才更新并重启；健康失败恢复原入口。实际激活成功，激活本身0次模型调用。
- WSL `/root/CosyVoice/docker/patches/nai_condition_cache.py`与Dockerfile COPY、两个服务入口同步，当前容器内模块同步。未重建镜像；容器重启可用，今后按已更新Dockerfile重建。未升级依赖或更改模型精度/权重、Windows安装版和用户模型配置。

## 两次本地真实调用（无自动重试）

1. miss-01：Rust→GPU，65280样本/68帧、PCM摘要与释放一致。缓存miss1/hit0，实际存58016字节。Root首PCM12362ms；服务观察首PCM12567ms、参考frontend2317ms、首LLM作业约6163ms、首flow2441ms/hift2372ms。重启后的首次慢不只来自条件提取，不能称已解决全部冷态。
2. hit-av-01：同一服务参考条件命中，miss1/hit1/entries1/58016字节；阶段记录中_extract_speech_token为0次（条件提取被跳过），服务首PCM1640ms、首token105ms。Root首PCM1658ms、首验证帧3716ms；独立设备入口首声5609ms、结束7662ms。48960样本/2.04秒全播、51帧校验、时钟选择51/实际窗口提交50，最终停止clock、最大年龄27ms。使用既有2秒有界预缓冲。

两次调用分别2.72秒和2.04秒、冷态与暖态不同，入口范围也不同，不计算加速百分比、不称稳定首声5.6秒。Root、服务及设备起点不同，不混减。日志与报告：`stages-before.json`、`stages-after.json`、`cache-after.json`、`activation.json`、`miss-01/report.json`、`hit-av-01/report.json`。全部本地，共2次模型调用，无付费供应商、不录音/推流。

## 后续

当前诊断仍每次启动GPU owner进行维护预热，再计业务时钟；5.61秒不含该维护预热，不是冷启动一键开播耗时。应先把已通过链路改为显式准备的常驻装配，并提供本地音色/模型有界预热状态，避免首个观众问题承担冷态成本；再接工作台正式入口。保持一个TTS、一个GPU worker和一个有界预取槽。GPU持续吞吐不足仍未解决，不以扩大预缓冲或重复跑测试替代优化。
