# CosyVoice3 真正流式服务交付

日期2026-11-07。完成C02/C03：仅更新API容器入口，WebUI未停止或重启。未改Rust运行时、数据库、模型权重或file_utils.py。真实本地合成2次、无重试、不播放、不推流，使用项目自带参考音频，仅为技术测试。

## 交付与验证

prepare-cosyvoice-streaming-patch.py由实际入口生成独立候选，对五路结构和stream透传严格校验，未知版本拒绝。保留opencode上传/Tensor修复，增加Form(stream=True)并传给模型、实际采样率/声道/pcm_s16le头、有限值/单声道检查、饱和转换和NAI_TTS_CHUNK/END日志。stream=False仍可显式选择，这是当前API参数，不是旧协议兼容层。

test-cosyvoice-streaming-patch.py在容器现有环境执行4项测试通过，含五路各True/False共10个子场景、上传16k链保持、先产首块再执行后续模型步骤、后续异常、非有限值和立体声拒绝。测试提取真实候选函数，以假模型验证路由和输出；不作为真实推理证据。OpenSpec严格校验通过。

激活前核对源hash并备份至E盘target-windows/cosyvoice-stream-validation。原入口SHA256 e87cd59ac552357dcabbe54711ddc25c35131d974e0b95e247c195c407b8f35f，现入口dd052053707daf0c41c99513b5e309a177370e57927e1e3176872f0ec5a66b30。file_utils原hash dfe3b2ce4f73915d39bbca330ae578b98f74c9f6ca9cb865e829f2232c35cc51 保持不变。

服务新入口存于/root/CosyVoice/docker/patches/server.py，Dockerfile.sm120增加对应COPY。当前容器写层入口已替换并重启成功，Windows openapi200；本轮未构建新镜像，若用旧镜像重新创建容器仍可能恢复旧入口，应按更新的Dockerfile重建。原file_utils只读挂载未改。E盘保存activation.json、原Dockerfile、原入口与原file_utils，恢复需明确选择备份，不以失败后静默降级冒充流式通过。

## 两次真实Windows测量

两次使用相同输入、同参考音频、stream=true、单次60秒总期限/10MiB输出上限；客户端不使用代理、不重试、无答案或PCM缓存。Python probe从Windows实际multipart上传并read1接收PCM，HTTP头不计首音频。参考指纹c7b31d6dbe7cc6a716dded00550db5b50940bf209e424e4ad207b12e657c8ff6。

| 指标 | API重启后首次请求 | 热态重复请求 |
|---|---:|---:|
| HTTP头到达秒 | 0.1743 | 0.0203 |
| 首可解码PCM秒 | 6.6306 | 3.9155 |
| 总完成秒 | 10.7286 | 5.4944 |
| 生成音频秒 | 8.16 | 7.16 |
| 整次RTF | 1.3148 | 0.7674 |
| 模型输出块秒 | 1.36 / 2 / 4 / 0.8 | 4.36 / 2.8 |
| PCM总字节 | 391680 | 343680 |
| 最大网络read间隔秒 | 2.5954 | 1.5692 |

模型日志4块/2块分别核对总样本195840/171840，与客户端PCM字节一致，确认不是完整音频完成后切片发送。网络read次数不等于模型块数；模型日志与Windows各自采用本地计时，不能拼接不同计时域计算子阶段。音频长度不同源于实际生成非确定，不能以这两轮宣称固定加速倍数。首轮不是从模型加载前开始的完整冷启动测量；热态仅一个样本，没有P95或长期稳定证据。

原始数据：target-windows/cosyvoice-stream-validation/windows-stream-first/result.json、windows-stream-warm/result.json，各有speech.wav；model-blocks.log含模型块证据。当前容器内存采样WebUI2.21GiB、API8.284GiB，不替代峰值/显存/FlashHead联合测量。

## 明确未完成与下一卡

定位到具体跨请求缺陷：CosyVoice3Model继承的tts中每块把self.token_hop_len递增，实例初始25/max100；首轮结束后未恢复，热态首块变为4.36秒。并发还可能共享此参数。下一卡C07应做每请求块长局部状态/可配置初始策略与单飞准入，不用每次重载模型解决。该问题目前仍存在，不能称首响优化已闭环。

C04 Rust表单/PCM适配和增量执行环、C05取消及共享GPU真实音画、C06输入限额/错误终止与并发硬化仍未完成。HTTP客户端断开不证明GPU停止，当前上游线程循环不具备已验证的取消树。服务尚无可靠可辨识的应用EOS/失败控制协议，HTTP200/正常EOF不是生产完整性证明。

结论：服务端真实流式和Windows上传接收已打通；实际设备播放、音画同步和M0没有因此通过。34项ACC保持不变，OpenSpec仍活动，不归档。
