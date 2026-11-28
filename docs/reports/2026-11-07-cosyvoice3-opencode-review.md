# opencode CosyVoice3 部署复核

日期2026-11-07。本轮只读复核；没有执行新合成、修改/重启/停止容器或部署先前候选补丁。以本报告更新早前部署初查的现状，早前失败证据保留。

## 已独立确认

- cosyvoice3/WebUI与cosyvoice3-api/API均存在并运行，restart=unless-stopped。WSL的Docker服务enabled且active。
- 两容器都只读挂载/root/CosyVoice/docker/patches/file_utils.py到模型工具路径。宿主补丁与API容器文件SHA256一致：dfe3b2ce4f73915d39bbca330ae578b98f74c9f6ca9cb865e829f2232c35cc51。Dockerfile.sm120第45行COPY同补丁，确认该构建入口持久化；其他Dockerfile/启动方式不自动覆盖为已验。
- Windows直连http://127.0.0.1:50000/openapi.json返回200，之前不可达已不再复现。尚未从Windows跑真实上传合成，不能据GET成功判端到端语音通过。
- 模型配置sample_rate24000。FastAPI上传使用multipart，返回int16裸PCM；现有代码astype(np.int16)是本机字节序，在本机x86为小端；未来协议应显式指定s16le。
- 容器日志有真实zero_shot提交和4.84秒音频，整次约10.32秒，模型日志RTF1.845不含全部前处理。这是原有日志证据，不是本轮新增实测，约2.13的整次RTF只适用于该样本。
- 当前容器内存采样WebUI3.611GiB、API7.924GiB，合计约11.535GiB；WSL上限约15.33GiB。GPU全设备用量7629MiB/24463MiB，未按进程归属，不能把容器内存当显存，也不承诺停WebUI必定释放6GB。

## 未闭环及审视意见

1. **模型流式未开启**：实际server.py中inference_*未传stream；当前模型方法stream=False默认值不变。StreamingResponse可以分句返回，但不是单句模型块级流式证据。后续应在现有修复基础上仅增加明确stream参数透传，再测首可解码PCM、块间间隔、总耗时和音质。
2. **本机吞吐仍需优化**：已有样本约10.32秒生成4.84秒音频，不能支撑无缓存持续讲解。缓存/预热可改善首响和复用，但不能抹掉未缓存RTF大于1；应在只保留所需API实例、复用参考音色特征后重新测共享GPU负载。是否停WebUI由后续性能测试范围决定，本轮没有停止。
3. **自启仅部分验证**：unless-stopped和Docker enabled已确认；Windows计划任务筛查没有发现名称包含wsl/docker/cosy的任务，这不是完整启动项审计。Windows重启后的WSL→Docker→容器→模型就绪尚未真实验证。手工stop的容器不会因unless-stopped自动恢复；不要称整机开机链已实测。
4. **Tensor采样率约束**：当前load_wav对Tensor输入固定认为16000Hz，与现有API先转16k再传的链条一致，但不能用于未知采样率的任意Tensor。未来应在边界明确输入采样率，避免把24k Tensor误当16k；当前不是证明API音频一定错误。
5. **接口生产接入缺口**：实际采样率/格式声明、参考音频与文本限额、并发准入、断开取消、生成失败后的可辨识终止、模型/音色身份和缓存失效仍需设计并实测。HTTP200可能先于模型失败，不能据状态码或空文件认定成功。
6. **Rust未适配**：不能沿用旧Qwen JSON协议；CosyVoice需要专用表单/PCM Adapter。现有执行端完整PCM/EOF准备门槛仍需改成有界增量PCM才能边收边播。FlashHead联合实时及34ACC不因本报告升级。

## 下一张完整任务

基于已修复的最新file_utils.py保持其持久挂载，不重做上传兼容补丁。先交付“显式模型stream透传→实际音色/采样能力→一次真实短句首块/末块及取消能力探针”的服务闭环，再按契约接Rust有界PCM播放。准备阶段产生的prepare-cosyvoice-streaming-patch.py及测试脚本未部署、未通过定向测试，针对旧入口准备，不可直接套到当前部署。当前C02/C03仍未完成。
