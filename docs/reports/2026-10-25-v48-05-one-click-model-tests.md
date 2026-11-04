# 一键文本/语音模型测试

日期2026-10-25，V48-05增量，ADR0170。用户要求移除测试金额上限及费用勾选的前后端逻辑，失败/欠费直接反馈。本轮没有调用真实付费API，没有读取/输出用户Key。

## 交付

- LLM/TTS模型测试均为点击即测试，移除金额输入、费用勾选、确认状态/金额校验及测试结果预留金额展示；成功语音仍手动试听。
- Protobuf先修改并生成：LLM使用专用FoundationModelTest，TTS删除max_cost_minor并reserved字段号5。测试请求不携带金额，不用隐藏默认上限替代。
- 新测试不计算金额预留、不校验工作区预算、不创建budget_reservations；单价未知、工作区禁用预算也可测试。固定测试内容、输出/期限、角色、模型版本、单飞、取消及不重试不变。
- 发送审计request_started及LLM usage保留；费用未知不表示免费。旧数据库、模型、Key和既有预算/未知费用记录不删除、不自动释放。业务生成/语音预演的预算与来源守门保持原有语义。
- 传输对有界错误体做有限分类，贯通持久任务和UI：明确余额不足、认证、权限、接口/模型不存在、限流/配额、服务异常、网络、超时、格式。普通403不凭状态推断欠费；不展示或存储原始错误响应/密钥。
- 保留上轮TTS提交超时恢复与原任务核对；不自动重试。失败时没有可试听音频。

## 匹配范围验证

| 范围 | 结果 |
| --- | --- |
| 存储组件 | 114/114；零预算/未知单价可测试、无新增预留、角色/模型版本、取消/重启/迟到、错误保留/脱敏；业务预算拒绝及结算回归保留 |
| 传输组件 | 10 lib + 5 TTS integration，通过；有限错误分类、本地HTTP、格式/取消/期限/不重试 |
| 界面 | 18/18；两个真实React测试面板均无金额/勾选、初始渲染不调用API、无音频无试听、错误提示、等待恢复与业务试听回归 |
| Windows原生 | 新错误矩阵1项（2类模型×6响应，共12场景），另LLM成功、TTS成功、取消迟到各1项，共4/4 |
| 契约 | 69条Rust→TS→Rust互通，229条消息全语言生成无漂移 |
| 静态/构建 | 前端类型检查/构建、相关3包all-targets clippy -D warnings、相关消费者check、引擎/输出/试听/桌面壳构建及GUI/认证IPC smoke通过 |

所有HTTP均为本地测试服务，Key为隔离生成的synthetic数据。新原生矩阵使用零预算工作区及未知单价，验证402、401、普通403、明确余额不足403、429、503；不创建新预留且不自动重试。测试通过不表示真实SiliconFlow/Agnes已通过。

存储回归暴露一条旧公共输出夹具缺seq/challenge/standby_frames，修正为符合现行healthy条件的夹具，没有放宽生产守门。Windows链接出现短暂LNK1104，原目标重试后通过；以.retry成功日志为准。未重跑全workspace，不改164ACC状态。

## 运行版与证据

用户确认关闭后已统一更新独立新线引擎/公共输出/试听及桌面壳，认证启动检查通过；完成后重新打开新备播工作台。旧开始菜单程序和原用户数据不变。

证据在忽略目录.nai-diagnostics：model-catalog-native-model-test-direct-{storage.log.retry,provider.log,check.log,clippy-final.log,bins.log,native-errors.log.retry,native-llm.log,native-tts.log,native-cancel.log}、model-test-direct-ui-final.log、model-test-direct-schema-check.log、model-test-direct-smoke.log，以及playback-wire-{write,verify}日志。只归档工程增量，不提交媒体/密钥。

真实付费Provider可用性、原真实TTS失败根因和V48-05业务音画仍需实际证据。当前点击测试会真实调用所配置供应商；无需本地金额/确认门槛，实际费用与供应商余额限制由供应商处理。
