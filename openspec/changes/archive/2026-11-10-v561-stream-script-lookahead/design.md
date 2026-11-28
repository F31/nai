# Design
后续优化（2026-11-10）：native-05的未来源晚158/303ms，说明等待整句GPU RenderComplete太迟。改为当前源经样本数/哈希与真实EOS验证、PCM门控封口后释放单次预取permit；该阶段仍不授予播放完成。下句TTS可与当前GPU尾部/设备播放重叠，但GPU保持单任务、TTS保持串行、未来源仍容量1及取走准入。取消/200ms时钟/45秒期限不变，不增加当前句预缓冲制造收益。使用独立运行目录最多三次本地合成验证；性能断言保持，失败保留。
原生验收前置修复：native-04 faulthandler定位VAE safetensors mmap读取在torch.storage访问冲突，0次TTS。本机微型文件验证官方pread后端可用。增加显式safetensors_backend配置（mmap/pread，缺省mmap）；适配仅包围单worker模型初始化、finally恢复库函数，不修改site-packages或权重。候选配置使用pread；不把微型读取成功当作完整模型或播放验收成功。native-03实际1次调用，累计3次；后续诊断最多再3次，保留失败，不自动重试。
复用Policy.split的标点/数字/引号边界和已有句数、文本、样本限制。一个TTS生产线程：首句经容量1分块通道送现有GPU增量循环；EOS验证后准备下一句到一个容量1已验证源槽，并等待该源被实际取走的permit，才开始再下一句，故最多当前句加一个未来句。每个源绑定整段Operation，停止后即使有旧PCM也不能消费。

抽取generate_source通用源入口，原generate_media仍代理本地API；新源只改变PCM来源，不复制GPU或音频算法。每句实际设备完成后消费下一源，同一PreparedSession复用模型；TTS可预取，GPU暂不跨句并发。首句不等完整合成，后续不二次合成。Source Drop清理PCM，Pipeline Drop撤销父操作并join生产线程；空通道断开不是EOS。

首轮真实验收首句失败、已有2次本地请求，保留native-01。TTs阶段记录显示首包8.66秒/整句18.53秒、下一句首包9.73秒/10.99秒，无法据此唯一归因输出失败。增加可信GPU RenderComplete＋PCM封口后的单次render-permit，当前句GPU计算完成才启动下一次TTS；后续源仍需被取走才可继续。这样预取利用设备播放窗口，避免下一句TTS与当前GPU尾部竞争；首次同句的流式TTS/GPU并行保持。原200ms时钟与45秒整段期限不放宽。补安全失败码，修复后再独立三句验收，连首轮最多5次本地请求，不自动重试。

native-02在准备阶段失败、0次TTS；Windows Event1000确认项目Python在torch_cpu.dll访问冲突0xC0000005，不冒充TTS失败或内存不足。追加PYTHONFAULTHANDLER和准备失败阶段/退出码留存，不改Torch版本、CPU线程或模型算法；native-03再次独立验证，模型调用总上限仍5，不称已修复原生崩溃。

Live控制开始使用脚本协调器，整段45秒上限不放宽，每次网络仍消费子操作期限，返回整段累计设备样本。记录源ready与设备完成回执的同一单调时钟，不能用不同origin推断重叠。定向单飞/槽位/取消/内容完整性测试、工作台类型构建和最多三句本地真实整链，失败保留；插问能力本轮不实现。
