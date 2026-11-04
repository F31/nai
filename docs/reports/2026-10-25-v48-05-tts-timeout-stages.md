# TTS超时阶段与模型测试等待

2026-10-25，V48-05增量，ADR0171。用户反馈一键测试返回DEADLINE_EXCEEDED。本轮改进期限及诊断，**没有重新发起真实合成，真实故障根因与Provider可用性仍未证实**。

## 发现与只读排查

当前Windows系统代理已开启，环境HTTP_PROXY/HTTPS_PROXY/ALL_PROXY未设置。只读取启用状态，没有修改系统代理，也没有读取用户Key。

无Key HEAD固定目标https://api.siliconflow.cn/v1/models：curl直连约123ms、显式回环代理约111ms；新增同一生产reqwest栈的network_check example，自动系统代理105ms、直连129ms，均收到HTTP404。HEAD未调用audio/speech，不涉及合成费用。404只表示收到HTTP响应；不证明模型目录GET、认证、余额、POST合成或后端排队正常，不能把15秒或代理直接定为本次真实故障根因。

现有TTS把连接、供应商响应及完整音频下载均压成泛化超时；固定测试总期限15秒，不能辨识具体等待点。[reqwest默认读取系统代理](https://docs.rs/reqwest/0.13.5/reqwest/)；[total timeout覆盖连接到响应体结束](https://docs.rs/reqwest/0.13.5/reqwest/struct.ClientBuilder.html#method.timeout)。本轮实测与这些配置语义分别记录，不从HEAD推导合成可用。

## 修改

- core定义MODEL_TEST_DEADLINE_MS=45000、PREVIEW_DEADLINE_MS=15000。root只对模型设置TestTts选择45秒；业务预演维持15秒，监督收尾为本次期限加250ms。连接3秒、取消检查20ms、样本上限、来源TTL/媒体租约不延长。
- adapter最大允许45秒；发送连接超时、等待响应头超时、HTTP成功后下载音频超时有不同安全错误码。已取得的非成功HTTP状态保留，不泄露原始错误。
- 安全码经存储词表和已有task_reason返回界面；UI显示最长等待45秒及相应提示。未取消后台请求不会声称取消；不自动重试、不会返回部分音频。
- 固定只读network_check仅HEAD该公开域名，系统代理和直连各一次，不读库/Key，不发合成。没有增加依赖或跨进程字段，因此本轮不重新生成契约。

## 匹配范围验证

| 范围 | 结果 |
| --- | --- |
| 语音传输 | 6/6；静默响应超时、200头后音频停顿、取消/迟到、解码/上限、不重试、45秒配置可接受且本地正常返回 |
| 存储 | 1项，循环验证4个安全阶段码持久化/费用未知/无音频 |
| UI | 10/10；阶段提示、无金额/确认、一键测试、原任务恢复；类型检查/构建通过 |
| Windows原生 | 3/3：实际桌面owner+本地服务16秒延迟仍成功（16.97秒）；取消迟到；业务预演HTTP与撤销回归 |
| 构建/检查 | 相关3包all-targets clippy干净；引擎/公共输出/试听/桌面壳构建、GUI子系统及认证IPC smoke通过 |

长等待用例初次误用64请求演练模式，快速轮询触及其正常退出限制；诊断后只将该用例改为实际--desktop-owner模式，没有放宽生产或演练限制。同时给测试IPC写失败补充4096字节上限的子进程诊断。没有用放宽超时来掩盖该夹具错误。

没有重跑全workspace，没有真实付费调用/真实声画认证，164ACC保持pending。证据位于忽略目录.nai-diagnostics：model-catalog-native-tts-timeout-{provider,network,check,storage,bins,native-delay,native-cancel,native-preview,clippy-final}.log、tts-timeout-ui.log、tts-timeout-smoke.log。

## 运行版及后续

用户确认关闭后已统一构建新线程序，启动检查通过，完成后重新打开工作台。数据、模型、Key、历史费用及系统代理不修改。新测试仍需用户自行点击；后续若再次失败，应按新阶段码核对供应商记录，不能自动重试或凭旧单次授权追加调用。V48-05真实业务音画与OBS仍未闭环。

## 再次超时后的端口核对

用户指出127.0.0.1:7892。只读检查时Windows系统代理实际为回环7890，监听检查仅发现7890，7892连接被拒绝。进程/用户/机器范围没有HTTP_PROXY、HTTPS_PROXY、ALL_PROXY覆盖；新工作台启动链及传输源码没有硬编码7892。

语音端点无Key HEAD：直连404约140ms，指定7892失败约2秒。进一步以空JSON、无Key、无模型/输入向/v1/audio/speech发送无效POST（不能触发有效合成）：直连401约125ms、7890代理401约107ms。只证明该时点直连/7890均可获得未认证响应；不能排除带认证合成的供应商等待或代理特定行为。本轮未调用真实合成、未改代理。若某处手工配置7892，其端口确实不可用；但没有证据把nai当前响应超时直接归因该端口。下一步可由用户在代理软件对该域名选择DIRECT后做实际A/B测试，不继续盲目延长期限。
