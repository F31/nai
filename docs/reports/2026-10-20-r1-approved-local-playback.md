# 批准候选到本机设备（2026-10-20）

## 本轮交付

已接通桌面显式动作→同一已批准文本→API TTS→真实2D渲染→私有原始媒体→独立keeper本机音画提交；不再只能运行测试音。旧文件预览仍默认不播放。用户操作、限制和复现见 [本机播放指南](../local-playback.md)，隔离/授权设计见 [ADR 0134](../adr/0134-approved-local-playback-boundary.md)。

仅Windows人工辅助，默认设备15%增益、≤5秒句段；过长拒绝。每候选/模式独立幂等及预算身份；先文件预览再点击播放是两个显式TTS请求。批准有效30秒并在界面说明，候选期限仍生效。未确认硬件播放完成，不写PlaybackAck或提高registry状态。

## 实现与拒绝边界

- 源OpenAPI新增PreviewCommand.local_playback，源Protobuf新增LocalPlaybackMedia/Grant/Status；先生成Rust/TS/C++，未手改生成物。未知计数用optional，失败不假装0。
- 原始PCM/RGBA分文件、固定文件名、有界尺寸/时长/SHA256；授权再绑定描述符SHA。根临时目录在成功/取消后回收，帧最大64MiB，无媒体JSON/Base64控制传输。
- 读取真实持久化审批，并校验actor/scope/expiry列一致；批准时持久化canonical cue事实集。实际复检审批/知识/事实/epoch/权限和Preplay守门，非Windows或缺侧车在TTS前拒绝。
- 设备初始化后才签发≤600ms租约，每100ms复检续期；root每10ms监督换代/停止，IO与观察队列有界，调用/初始化/回收有期限。keeper队列4+1，200ms音频块，回调逐片检查短租约、P0和预测设备排队时间。
- 子进程清空继承环境，只保留必要Windows路径并排除Provider凭据变量；不继承API密钥。子进程状态封闭校验后才记录，不接受伪造硬件完成或诊断注入。
- 本机窗口按音频提交样本位置选择真实RGBA帧；静音按钮不受busy阻塞，旧代次迟到结果丢弃。状态区分文件rendered、设备playing/submitted与failed，不把设备提交当实际完成。

## 实际Windows链路

在本机Windows/MSVC执行显式真实进程测试。Provider是本地HTTP协议fixture，未访问付费账号；engine、Store、2D渲染、继承管道、WASAPI及窗口均实际运行。

已验证正常链路24000个48kHz单声道等效样本、13个真实渲染帧提交；先文件预览再播放成功。另在已观察到1056个样本提交后执行静音，任务以STALE_EPOCH失败，临时媒体清理。重复播放命令不再次TTS；未批准拒绝且不TTS。

独立原生故障测试验证帧文件篡改在输出前失败；已开始输出后EOF与不续租均未完整提交，不发ACK。测量的是状态返回/进程收尾时间，不以此替代硬件静音时刻。硬件故障退出的缺失计数表示未知。

## 修复的真实接线问题

- 二进制ready帧受stdout行缓冲滞留，导致等待授权超时；每个状态帧显式flush修复，未延长期限。
- 文件预览与本机播放原共用预算request_id，现按显式模式分别标识，避免正常“先预览后播放”撞既有账本。
- 预览rendering阶段原返回空摘要字符串，被前端严格解析拒绝；无摘要时省略可选字段，并按revision分开两种模式结果。
- 现有候选未绑定守门policy_version；新生成候选在hash之前绑定当前规则版本，旧空版本不事后补填绕过批准。
- 本机协议回归首次因继承代理超时，清除代理后14/14通过；没有改Provider为假成功。
- 加严回调提交前的真实单调时间复检后，发现正常结束P0被误归为设备故障；将主动撤权清零与真正截止期错误区分，完整播放及播放中静音复测通过。
- 前端生成DTO要求显式布尔值，现始终发送false/true；批准期断言同步30秒，109项前端检查通过。未修改生成代码或放宽类型检查。

## 未完成

真实付费TTS/LLM认证、听感/音画延迟和设备矩阵、虚拟设备/直播工具、硬件完成ACK、正式签名与命名管道ACL对端认证仍未完成。窗口与root同时卡住时的画面独立失联待机未认证。API/素材实际质量由真实配置决定；fixture与微型测试素材不能冒充业务内容或形象验收。

## Provider线上格式修复

核实[OpenAI官方语音文档](https://developers.openai.com/api/docs/guides/text-to-speech)：裸PCM为24kHz、16位小端。适配器此前回显请求采样率，实际请求无采样率参数，会导致真实语速/时长错误。现在响应与流块标注真实24kHz mono，引擎用已有有界重采样转换到渲染采样率；WAV、媒体文件和摘要均基于转换后的真实字节。回归请求8kHz而响应必须24kHz，端到端fixture使用12000个24kHz样本，验证输出4000个8kHz渲染样本和24000个48kHz设备提交样本。

## Windows安装产物与复测

新工程安装包：target-windows/shell/x86_64-pc-windows-msvc/debug/bundle/nsis/nai Engineering_0.1.0_x64-setup.exe；8,512,422字节，SHA256 681efb7b900b48bad3cc9485bbc6b98f5776e15214f319a305d1c92f9a50d660，NotSigned。实际安装、活动进程/覆盖/备用目录拒绝、随包预检、页面加载与卸载通过，显式保留既有测试数据，SQLite等3文件哈希不变。安装测试未再次测设备，设备证据来自独立原生测试。

复测发现旧测试留下卸载器与空备用目录；仅归档本任务残留卸载器、移除已核实空测试目录后重跑。脚本将备用目录新鲜度检查移到任何安装动作之前，防止部分执行后才拒绝。未删除用户数据。PowerShell语法检查及Windows engine/keeper all-targets clippy -D warnings通过。


## 最终工程检查

cargo xtask check --profile full 最终复跑EXIT=0：107个Rust目标、1228 passed/0 failed；Node合计453 passed/0 failed（前端109）。17类Protobuf消息Rust/TS/C++互通及schema --check通过。keeper lib 14项通过；Windows显式完整播放/播放中静音与四类原生故障均通过；Windows all-targets clippy干净。

检查日志：WSL /tmp/nai-approved-playback-full-final.log、/tmp/nai-approved-playback-contracts.log、/tmp/nai-approved-playback-schema-check.log；本机 .nai-diagnostics/approved-playback-rate-native.log、approved-playback-native-faults-descriptor.log、approved-playback-final-clippy.log、approved-playback-install-final.log。生成C++固有尾随空白未手改；排除生成目录后git diff --check通过。改动未提交。

OpenSpec已归档为2026-10-20-r1-approved-local-playback；43个主规格严格校验通过，164项归档的提案一致性3项测试通过。
