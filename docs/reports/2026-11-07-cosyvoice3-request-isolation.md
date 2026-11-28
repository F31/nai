# CosyVoice3 每请求块长与单飞闭环

2026-11-07，C07工程与目标机服务验证完成。只更新API容器，原上传补丁与WebUI保持；Rust、数据库、模型权重和34项ACC未修改。本轮无付费调用，4次本地模型调用（3次完整，1次客户端主动断开但后台排空），另1次HTTP并发请求被409拒绝，未进入模型；没有合成重试或设备播放。

## 改动

- prepare-cosyvoice-request-isolation.py按已知源码结构生成补丁，编译后输出独立候选，不直接部署。模型CosyVoice2Model.tts中增长块长改为请求局部变量，CosyVoice3继承消费。初值来自模型base hop，COSYVOICE_INITIAL_HOP_TOKENS可选不超过max的base整数倍；默认25，max100来自当前模型，不硬写进Rust。
- AR工作线程异常回传至生成器，结束标记在finally写入。生成器正常/失败/早关都等实际线程退出再清理三个请求字典，避免线程仍写已删除状态。保留上游既有CUDA清理策略，没有同时更改模型参数或画质。
- 新stdlib模块cosyvoice_request_gate.py提供单个模型工作线程、两块有界PCM交付队列及每块大小上限。模型忙时返回409、不排队；health不等待GPU锁。生产者结束并完成清理才释放准入。输出校验异常会显式关闭上游生成器。
- HTTP断开或交付期限到达后停止交付，后台在原占位下排空；不把客户端取消等同GPU取消。卡住的后端不释放准入，期限后health为blocked。COSYVOICE_REQUEST_DEADLINE_SECONDS默认60秒；真正GPU取消仍未实现。

## 定向测试

容器现有依赖执行10项隔离测试通过：连续三请求初始块不变、显式初始配置/非法配置、AR失败、解码失败及早关字典清理、并发拒绝、断开保留准入直到实际结束、期限后拒绝新任务、异常后恢复、队列/字节上限、真实Starlette ASGI断开行为。模型方法取自实际候选AST，使用CPU桩而非加载GPU模型。

5项接口/输出测试通过，含五路接口stream True/False共10个子场景、原16k上传链保持、首块先于后续生成、非有限值/立体声拒绝及校验异常关闭上游。它们是机制测试，不算真实设备证据。

## Windows实际服务验证

同一段文本及自带参考音频，24kHz PCM16LE、60秒/10MiB有界探针、无结果缓存。候选V2进行了连续/断开/恢复三次；最终V3只补上游close清理，模型与gate代码未变，再做了一次完整冒烟。没有重载模型来重置同一轮连续请求的块长。

| 请求 | 首PCM秒 | 总秒 | 音频秒 | 整次RTF | 模型首块 |
|---|---:|---:|---:|---:|---:|
| V2重启后首次，含并发拒绝验证 | 4.9718 | 9.0022 | 8.16 | 1.1032 | 1.36秒/32640样本 |
| V2热态，收到首PCM即断开 | 2.6757 | 主动断开，不作完成 | 后台7.16 | 不计算 | 1.36秒/32640样本 |
| V2排空后恢复，无重启 | 2.4383 | 7.4030 | 8.72 | 0.8490 | 1.36秒/32640样本 |
| V3最终入口重启后冒烟 | 4.7209 | 9.4427 | 8.16 | 1.1572 | 1.36秒/32640样本 |

真实并发请求409耗时33.84ms，模型日志没有新增对应生成。断开后31次health观察到了busy=true与delivery_cancelled=true，约3.46秒后idle；该期间健康查询最大29.2ms。这是停止交付并排空证据，不是GPU抢占停止或硬件静音。断开请求只接收1454字节，未发布可用完整音频。

所有首块恢复同一1.36秒输出策略，不再跨请求变成4.36秒。该1.36秒包含参考音频对齐pad，不能将25token简单等同所有请求1秒。热态首响仅单次有效样本，音频长度非确定；不能宣称P95、固定加速倍数或持续直播通过。最终重启首轮仍约4.72秒，持续性能仍需后续优化。

## 维护与原始记录

备份/候选/activation.json/activation-final.json及探针记录统一在E盘target-windows/cosyvoice-request-isolation；first、disconnect、after-disconnect、final-smoke各有result.json，完整请求有speech.wav；disconnect-health.json保留全部健康采样，model-blocks.log保留模型块记录。

最终server SHA256：70fcc100da27490474914c599c91cdccca3070ffb1b5842d2be546e98bc31ade；model：35603baaf324825702e7f2f157f9da42897c68f1421d28a89a9b3676945e7e72；gate：c4b1fcd384d0347a873deaa4ea62b1a6cd9e52527527996f53eb1608b0c40679。原file_utils只读挂载及hash不变。

宿主docker/patches中保留server.py、model.py、nai_request_gate.py，Dockerfile.sm120增加对应COPY。当前容器已激活；本轮未构建新镜像，旧镜像直接重建容器会回旧实现，需要使用更新的Dockerfile构建。API单独重启，未修改其他容器或Windows网络。

## 下一卡

C04先定义/生成CosyVoice配置和增量PCM契约，再做Rust表单/流式Adapter→有界增量PCM执行环→实际边收边播。C05生产取消树/缓存身份/FlashHead联合音画、C06上传/文本限制/应用EOS与失败协议仍未完成；单飞部分已提前完成。HTTP200或正常EOF仍不能替代生产应用EOS/内容完整性证据。OpenSpec保持活动，34ACC不提升。
