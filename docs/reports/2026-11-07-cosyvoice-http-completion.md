# CosyVoice HTTP完成回执与Rust守门

状态：C05d2a已实现、定向实测并更新API；真实模型到独立播放的本轮联测尚待后台ppts暂停。C05整体、缓存、FlashHead/OBS和34项ACC均未提升。

## 行为

HTTP契约升为1.1.0，正式生成Rust/TypeScript DTO。音频响应携带每进程随机instance与每任务随机request ID，新增同源完成状态端点。服务在模型迭代和清理后锁存样本及SHA256；内存记录最多16条、120秒，不存音频、不建持久账本。限额拒绝不会排队生成。现有spks与上传/音色ID分支均保留。

Rust继续增量交付PCM，但仅在HTTP正常结束且回执completed、instance/request/样本/摘要全部相符时才允许诊断Seal和完成结果。未知、过期、失败、仍运行或不匹配均拒绝；回执正文最多4096字节，原Operation取消与期限仍有效，没有自动重新合成。传输EOF与随后回执校验的耗时保持区分。CLI成功报告增加application_eos_verified字段。

这不撤回已经流式播出的声音，不声称音色/权重身份、硬件播放、公共输出许可或无间隙直播通过；缓存尚未发布。

## 验证

- Rust HTTP 7项：增量及奇数边界、编码/截断、忙拒绝/样本上限、父取消、回执等待取消、缺身份、404/不一致/额外字段及4096字节限额；含多种故障子场景。
- Windows原生独立音频4项、宿主完成/停止/迟到/关闭4项通过，均为静音夹具。
- Python生成状态8项、回执边界4项、容器模型/准入CPU替身10项、ASGI正常及部分音频异常1项通过。
- Node配置/HTTP契约2项、clippy、18包依赖边界、契约生成无漂移及OpenSpec严格检查通过；注册表34项仍pending。
- HTTP测试夹具曾因Windows accept继承非阻塞模式失败，修正accepted socket模式后回归通过；不将其归因供应商。

## 部署与真实联测边界

API入口修改前已快照并核对当前SHA256；复用当前镜像，不下载或重装模型。保留原容器cosyvoice3-api-before-v561-eos、源文件、环境、挂载与回滚记录；新代码只读挂载。Dockerfile.sm120新增模块COPY并保留原文件；没有构建新镜像。激活后健康ok/busy=false，8个已有音色逐项比较不变。

起初busy持续存在，用户确认没有手动使用后做维护恢复。重启后继续发现请求；通过WSL连接归属定位到ppts（当时PID95333），日志显示它持续正常完成合成。故不能将此前busy判为模型死锁；已告知用户，未终止ppts或再次重启服务。本轮尚未从nai发起真实模型验收，不能把ppts的合成记录算作nai通过。

维护记录位于target-windows/cosyvoice-completion-acceptance/service-swap/activation.json，代码/日志/缓存保留在E盘；未改旧数据库、Key或桌面壳。

下一步：待ppts暂停后做一次有界真实合成与回执/播放验收；随后补模型/参考音色/文本身份指纹和完整音频缓存，缓存命中须证明无新模型POST，再接FlashHead同源音画与OBS。
