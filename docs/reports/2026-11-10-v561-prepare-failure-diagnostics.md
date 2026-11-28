# V5.6.1 准备失败诊断与安全反馈

## 结论

错误分类及反馈工程实现完成，正常准备恢复和桌面维护交付未完成。不能把GPU低占用解释为算力不足，也不能把同步诊断成功当作正常工作台恢复。OpenSpec保持活动，34ACC及M0 No-Go不变。

## 实际证据

- 原工作台session-1794299924512737900的prepare-1/2均在Wav2Vec2特征提取GroupNorm预热阶段报告CUDA unknown error。没有准备Ready，因此“开始讲解”禁用符合状态约束；原Root丢弃具体失败原因，只显示PREPARE_FAILED。
- 同Python的小型CUDA GroupNorm运算成功。显式CUDA_LAUNCH_BLOCKING同步全配置诊断成功，73.407秒；文件target-windows/v561-prepare-diagnostic-02.log。该调试变量未加入生产环境，第一次诊断脚本环境白名单错误及失败日志保留。
- 新增零TTS、正常生产进程/认证IPC准备测试，target-windows/v561-prepare-normal-01。本轮真实测试失败，30.89秒，权重加载报告OSError 1455（页面文件太小），随后diffusers处理异常时MemoryError。旧运行Root只收到通用失败，不称恢复。
- 失败后只读检查：物理内存约31.4GiB，空闲约6.7GiB；系统虚拟内存可用约9.8GiB，C盘活动页面文件34193MiB、使用12085MiB；vmmemWSL私有内存19.36GiB。未修改页面文件或电源设置，未关闭其他应用、重启WSL或停止CosyVoice。前两次CUDA错误与本轮1455是否同因尚未证明。

## 实现

独立worker使用已有签名KIND_FAILED，在准备阶段返回固定错误码；CUDA错误、CUDA显存耗尽、主机内存/Windows1455、维护超时及其他模型错误分别分类。完整异常仅留在私有日志，不转发路径/异常正文。Root先验证MAC、事件版本/序列/形象身份，再将准备失败白名单写入有界诊断文件；控制界面读取固定码，失败不启用讲解。主动取消与期限耗尽分别为Stopped/Failed，没有自动重试和静默扩大期限。

没有新增协议字段、模型依赖或运行常驻服务，没有修改模型尺寸/步数/画质，没有启用CUDA同步调试作为运行修复。测试注入worker仅位于tests/fixtures，产品配置不使用它。

## 验证与交付

- Python分类与实际worker签名失败输出4项通过：无Ready、无raw连接、MAC及零任务身份验证。
- Rust准备模块4项通过：签名头/身份边界、真实Ready几何、PCM源边界及固定错误白名单/文件大小限制。
- React工作台1项通过，包括安全中文分类和未知错误不透出原文；TypeScript及Vite构建通过。
- 新Root/桌面候选构建通过。Windows原生故障注入1项通过，实际私有IPC收到HOST_MEMORY_EXHAUSTED且busy=false/visual_prepared=false，开始讲解被拒绝；该注入不是GPU恢复证据。
- OpenSpec严格校验通过。仅定向测试，没有全仓测试、TTS/LLM调用、音频播放或推流。
- 候选Root位于target-windows/v561-prepare-candidate，桌面候选为target-windows/v561-clock-build/prepare-diagnostics-workbench.exe。现有运行程序未替换。

## 待办

请操作者先释放无关内存负载，保留语音服务，再做正常路径准备复验；若资源仍不足，再基于峰值数据决定模型低峰值加载或系统内存配置。根因及正常恢复未证实前不归档、不授验收通过。保存退出当前工作台后，才能维护更新Root和桌面错误提示。

## 释放资源后复验与交付

操作者确认释放WSL无关资源后，只读检查可用提交内存约16GiB，无在运行工作台/模型。使用候选Root、原profile/形象/权重/模型参数及正常隔离进程环境，未启用CUDA_LAUNCH_BLOCKING，运行零TTS真实准备测试。target-windows/v561-prepare-normal-02测试通过，108.99秒；worker PID50984产生真实Prepared，visual_prepared=true，并正常退出。没有扩大原240秒维护期限、禁用编译或降低画质。恢复后的单次通过不证明之前CUDA错误与内存1455同因，也不证明长稳。

已备份并维护更新开发运行目录的Root/桌面两程序，SHA256核对通过，journal为target-windows/maintenance/prepare-diagnostics-20261110-215827/journal.json，status=activated；音频程序/配置/素材未改。新版工作台PID54460重开，不自动准备或调用模型。旧开始菜单安装不变。原生正常准备恢复和维护交付已完成；新版实际按钮人工确认仍待补，OpenSpec保留活动，34ACC/M0不变。
