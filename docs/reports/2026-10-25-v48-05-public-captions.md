# V48-05 公共字幕、输出状态与真实录制尝试

日期：2026-10-25。基线V4.8，ADR0169。**字幕与呈现状态工程交付；真实TTS/OBS业务音画未闭环，V48-05仍InProgress，164ACC全部pending。**

## 实现

- 先修改foundation.proto，再生成Rust/TS/C++及JSON Schema/OpenAPI；私有准备携带批准原文，keeper复算caption hash。窗口报告独立presentation，不把音频提交、窗口提交或OBS混成播放完成ACK。
- 固定系统黑体/微软雅黑，实际字体hash和明确失败状态；最多2048字节/256字符/三行，不改写、截断或替换缺字。fontdue 0.9.4仅用于Windows keeper准备worker，字体不打包，音频callback不解析字体。
- 首次源样本提交后显示完整本句字幕，P0、停止、撤销、失联、到期、完成恢复待机并清除实际presentation。实际提交成功后报告准备ID、帧、源样本、字幕和合成画面hash。
- 界面显示字幕准备/拒绝原因、窗口实际提交状态，并解释整句字幕及提交时钟的证据边界。
- 新增显式授权验收example public_av_acceptance：正常root/宿主保护IPC、单独标注演练工作区、共享已保存模型、不读出或复制Key；派发前全局单次账本、无自动重试。无RUN信号不发送API。

## 定向验证

| 范围 | 结果与边界 |
| --- | --- |
| 媒体库 | 46/46，含原文完整性、首样本前/停止/到期/完成不显示 |
| Windows字体 | 2/2，真实中文字体、篡改、缺字、溢出、取消；QA图已查看 |
| 在用界面 | 21/21；修正测试夹具两次取时造成的边界抖动，生产期限不变 |
| 原生公共执行 | 2/2，真实窗口/SDK提交与停止、撤销、root强杀、P0、仅健康续租五类故障；夹具是本地静音数据，不是付费语音 |
| 相邻原生回归 | 静音准备1/1、私有设备试听停止/撤销1/1 |
| 互通与生成 | 69条Rust→TS→Rust；228条消息生成检查无漂移 |
| 代码边界 | 相关7包all-targets clippy -D warnings、相关check、28包依赖检查通过 |
| 运行版 | 引擎、keeper、试听程序、桌面壳构建成功；GUI子系统与认证IPC启动smoke通过 |

完整workspace测试未重跑；不因局部实测升级任何ACC。桌面smoke需PowerShell 7，误用Windows PowerShell 5时缺少IsPathFullyQualified，换正确宿主后通过。

独立运行版已重新打开；保留数据库、模型、Key、原始OBS配置和旧开始菜单安装。OpenSpec按字幕工程增量归档为2026-10-25-v48-05-public-captions，归档不授予整项音画验收通过。

## OBS与唯一一次真实TTS

OBS安装源D:/obs-studio，33.0.0；独立副本在.nai-diagnostics/obs-caption-acceptance，保留原OBS安装/配置。配置唯一窗口捕获源nai-public-window-only，WGC匹配公共窗口及其应用音频；全局桌面/麦克风源为空，不推流。复制时遗漏core编码器、残留崩溃检测目录、输出路径转义问题均在试录前处理；没有为这些问题派发API。

12秒待机试录成功，361帧，提取画面确认实际公共窗口；WASAPI进程音频捕获初始化成功。正式录制由本地OutputTimer于60秒后停止并完成封装：512×288、30fps、H.264、48kHz双声道AAC，ffprobe时长60.10秒、1801个输出帧。**有效轨道/录制成功不代表真实业务音画通过。**

用户授权SiliconFlow/CosyVoice2 alex，原文“容量为500毫升。”，最多1次，预算0.10元。正式录制开始后派发唯一一次，结果：

```json
{"state":8,"reason":"SYNTHESIS_FAILED","charge_status":"unknown"}
```

任务1312c7cd-fdec-447b-9ac8-cea71325ff20。没有成功音频，没有公共业务执行，没有重试，不宣称未计费。全局.nai-diagnostics/caption-authorization-once.json在派发前已消费本次授权，换目录也不能绕过。现有失败存储丢弃具体错误，只保留通用码，因此**尚不能判断是网络、认证、供应商响应还是音频处理失败**，不能无证据归因余额/代理。

可见窗口前台工具承载时曾出现WB_DEADLINE_EXCEEDED，独立后台验收控制器就绪并持续服务后完成试录；这不授所有前台环境兼容通过。预派发尝试产生的演练工作区保留，不自动删除用户数据；实际派发工作区a56b25aa-9a3e-4213-8cfe-74e8227b7990，名称明确标注演练。未知费用预留不擅自释放。

## 证据位置（本机忽略目录，不提交媒体/密钥）

- .nai-diagnostics/caption-live-take4/{setup,dispatch-once,tts-result}.json
- .nai-diagnostics/caption-live/recordings/nai-caption-2026-10-25-11-35-14.mkv：待机试录
- .nai-diagnostics/caption-live/recordings/nai-caption-2026-10-25-11-36-23.mkv：真实合成失败的正式录制，仅证明录制链路
- .nai-diagnostics/obs-standby-frame.png、caption-qa.png
- caption-schema-check.log、caption-ui-tests.log、caption-media-regression.log、caption-deps.log
- model-catalog-native-caption-{font,core,clippy,check,live-driver-build}.log
- model-catalog-native-playback-native-{execution,preload,audition}.log；workspace-fix-shell.log

OBS配置采用[官方启动参数](https://obsproject.com/kb/launch-parameters)及[portable说明](https://obsproject.com/kb/portable-mode)，实际录制结果以本机日志和文件为准。

## 未完成与下一步

1. V48-05内补充脱敏、有限枚举的TTS失败诊断：区分传输/期限、HTTP状态、响应类型、解码及取消；不保存原始响应/认证头，使用本地服务测试，不需要付费重试。
2. 真实业务语音仍Blocked：本次唯一授权已用且失败；后续重试须新明确授权，或使用用户已有且符合本次来源守门的真实语音。不得删除单次账本自动重试。
3. 取得真实语音后补字幕可读性、口型变化、声音内容、停止/完成清除、音画偏移与OBS实录核验；扬声器主观体验和物理设备完成证据仍独立待验。
4. V48-06/07统一Harness、连续讲品、插问归位、真人麦克风接管未因此完成。V48-05工程字幕增量与完整产品验收分别跟踪。
