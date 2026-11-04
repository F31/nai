# R1 静默输出预热（2026-10-21）

## 本轮交付

直播台增加“准备直播输出（不调用API）”。连接后可先打开当前已准入形象的合法闭嘴窗口，建立一条静音音频stream；没有文本、TTS音频、候选、审批或语音租约。之后正常审批播放复用该输出进程、窗口和stream。

新增HTTP `POST /sessions/{id}/output-prewarm`，复用SessionCommand、私有控制适配和共享幂等账本，独立权限media.prepare。ACK只接收，既有events给出output_preparing/output_ready/failed；成功记录独立待机摘要，UI须等匹配任务完成并重读当前session/epoch才显示本次确认。失联/换代/超时不显示迟到成功，失败不自动重试。

新增LocalStandbyMedia和LocalOutputWarmup私有Protobuf。RGBA单独文件，描述符摘要绑定身份/epoch，独立像素摘要、文件类型和几何/16MiB上限；producer也在写出前拒绝超限。设备侧单次处理预算至多4秒，root整任务至多8秒，独立于普通控制请求的异步接收。root确认零语音样本、单stream、匹配中性摘要，任一失败回收未确认输出与临时文件。预热与渲染共用单槽准入，忙时拒绝；P0仍用独立EmergencyHandle，不等待设备任务锁。已退出的输出子进程可在下一次显式准备时重新建立。

架构边界见 [ADR0139](../adr/0139-neutral-output-prewarm.md)。这新增独立合法中性准入，不借用语音preplay，不把零样本/打开设备写成PlaybackAck或平台Ready。

## 验证结果

- Linux/WSL定向模块：control-api lib **51**、output-keeper lib **18**、engine lib **13**，通过。覆盖独立权限、幂等/跨操作ID冲突、外设备拒绝、陈旧epoch，以及描述符/像素篡改、身份/预算/尺寸上限。
- engine真实进程本地协议fixture的operator_flow **20项通过**；Linux预热不可用在任何Provider调用前拒绝，新增断言单独复跑通过。fixture非真实付费Provider。
- UI相关 **36 passed / 0 failed**，包含新增预热3项：真实ControlClient限定命令、ACK不等就绪、只接受匹配任务和有效摘要、失败/换代/断连/结果缺失不重试。最终提示调整后新增3项单独复跑通过。桌面typecheck与Vite78模块生产build通过。
- Windows/MSVC原生 **3项通过**：keeper预热零样本、640×360待机后连续六段/取消/新epoch/帧篡改恢复仍仅一条stream；真实engine先预热且本地Provider的LLM/TTS请求均为0，相同命令幂等，后续两条独立批准候选复用同一PID，语音中静音撤权；预热已发命令后撤权回收尚未确认的子进程。
- Linux与Windows三模块clippy all-targets `-D warnings` 通过，fmt通过。最后producer写出上限在engine直接模块再检查。
- `cargo xtask schema --check` 无漂移。Protobuf Rust→TS→C++→Rust及Rust→C++→TS→Rust双向通过，**25 fixtures / 22 message types**，覆盖新中性描述符、warmup oneof、大整数epoch和中文路径。HTTP/配置契约Node模块 **37 passed / 0 failed**。
- OpenSpec已归档为 `2026-10-21-r1-output-prewarm`，resident-local-output主规格严格校验通过，归档 **181 passed / 0 failed**。改动文件空白检查通过。
- `cargo xtask check --registry`：**57 passed / 0 failed**，验收状态未变。

## 检查中修正

最初控制测试遗漏Content-Type、UI事件fixture缺revision，两项测试如实失败，补齐fixture后通过。契约闭包检查发现新HTTP路由未登记到已实现操作表，补入scripts/contracts/openapi-ops.mjs后37项全部通过。Windows链接器多次因测试exe临时锁返回LNK1104，重试同一测试后均通过，没有杀用户进程或跳过检查。未把“编译了ignored测试”当作实际原生执行。

本轮只测改动模块及直接消费者；没有全仓/完整发布组合测试，没有付费API、安装替换、真实用户库/凭据写入或Git提交。原生画面/语音均为明确测试fixture，不代表生产形象许可、真实TTS质量或OBS捕获认证。UI测试为客户端/组件层，本轮未做已安装GUI点击验收。

## 未完成和后续

当前仅关闭节目启动的输出准备子项。PitchRunner生产有限节目、结构化价格/活动时效及授权事实来源、金额预算、可核验的播放完成进度仍待接，整个自动讲品/插问归位未完成。真人麦克风采集接管、OBS实际录制仍未开始；保持原顺序推进。registry A01/R01/R03/B07等状态不升级。

静态素材和许可随引擎会话固定，变更需停止重准入；动态许可撤销、正式共享内存、命名管道ACL/对端认证、30分钟以上长播、待机微动作与强杀遗留清理仍缺。预热成功是本次点时观测，用户关闭窗口后需重新准备；不承诺窗口持续在线。当前安装版尚未更新。
