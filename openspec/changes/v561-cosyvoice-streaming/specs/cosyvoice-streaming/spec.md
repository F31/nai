# CosyVoice 本地服务

## ADDED Requirements

### Requirement: 实际部署身份和协议
系统 SHALL 将用户部署的CosyVoice作为当前接入目标，独立识别其表单协议及PCM格式，不能以旧Qwen配置或容器进程参数授予实际模型验证通过。

#### Scenario: Windows不可达
- **WHEN** WSL内部成功但Windows访问失败
- **THEN** Windows接入 MUST 保持未通过，独立记录两条路线的证据

### Requirement: 上传文件和流式生成
服务 SHALL 有界接收参考音频，以当前模型支持的文件形态传入并在完成/失败后清理。stream参数 MUST 传递到模型推理，而不是仅包装HTTP响应；返回格式和采样率须明确。

#### Scenario: 单句流式
- **WHEN** stream=true且模型支持增量生成
- **THEN** 探针 SHALL 验证首可解码PCM早于整句结束，独立记录音频长度、耗时及缺块，不以read次数替代模型流式证据

#### Scenario: 参考音频超限
- **WHEN** 上传字节数超过配置上限
- **THEN** 服务 MUST 拒绝并清理临时数据，不调用推理

### Requirement: 激活与证据
维护工具 SHALL 保留原服务入口并严格核对目标结构，验证通过后才激活；真实合成、取消和音画未测时不得授予通过。

#### Scenario: 未知服务版本
- **WHEN** 源码结构与补丁预期不符
- **THEN** 工具 MUST 拒绝输出补丁，不覆盖运行中的文件

### Requirement: 流式生成状态隔离
系统 SHALL 将递增块长和推理上下文绑定到当前请求，后续请求不得继承上次请求的末段块长；单GPU推理需要有界准入，拒绝并发不应阻塞健康和停止控制。此要求在C07完成前保持未实现。

#### Scenario: 连续请求
- **WHEN** 相同配置完成第一句后开始第二句
- **THEN** 第二句 MUST 使用声明的初始块策略，不继承第一句增长后的token_hop_len

#### Scenario: 客户端断开但模型仍运行
- **WHEN** 客户端断开或交付期限到达而底层推理线程尚未结束
- **THEN** 系统 MUST 停止交付并保留单飞占位直到模型实际结束，不把HTTP取消当GPU停止；期限后健康状态明确blocked，不接受另一推理

#### Scenario: 并发准入
- **WHEN** 已有推理占用单飞槽位
- **THEN** 新请求 SHALL 获得明确busy错误且不排入模型队列，健康查询不等待模型锁

### Requirement: 本机增量PCM独立执行
本机试听系统 SHALL 边接收边向独立音频进程交付PCM；控制与媒体分离，跨进程范围、序号和摘要通过每spawn能力签名绑定。接收端缓冲与整句长度 MUST 有界，失联租约独立到期；设备消费样本与网络接收、设备提交分别计数。

#### Scenario: 完成前播放
- **WHEN** 供应商尚未结束HTTP音频流且已有可播放PCM
- **THEN** 本机试听 SHALL 可启动设备消费，并记录实际设备源样本首次推进早于HTTP结束；该记录不授予公共播出许可或音画通过

#### Scenario: 断流与伪造控制
- **WHEN** 流被截断、范围/摘要/认证/epoch/序号不符或租约到期
- **THEN** 执行 MUST 拒绝继续播放或完成回执，不以已收到的部分音频冒充完整完成

#### Scenario: 缺料静音
- **WHEN** 设备在等待后续源音频
- **THEN** 插入静音 MUST 不推进源样本时轴；时间映射不得限于旧四段固定CueSpan，队列和未完成设备区间仍须有界

#### Scenario: 供应商缺少应用EOS
- **WHEN** 当前CosyVoice仅提供HTTP EOF
- **THEN** 本机诊断 SHALL 明确其传输完整性边界，不将本地Seal提升为供应商应用EOS、生产内容许可或无间隙直播证据

### Requirement: 语音控制入口与统一取消
桌面语音入口 SHALL 复用Rust单任务owner及父子Operation取消，停止不等待模型或数据库锁；取消确认前保持单飞，迟到完成不得覆盖取消结果。入口 MUST 与旧工作区协议/数据隔离，仅显式读取所选当前配置，不能放行尚未就绪的完整直播启动器。

#### Scenario: 生成等待时停止
- **WHEN** 用户在等待首PCM时点击停止
- **THEN** native控制 SHALL 先撤销owner，再有界回收音频进程，不能等待整句模型返回；UI区分取消请求和软件结束，不声称硬件静音完成

#### Scenario: 停止后旧完成到达
- **WHEN** 已取消任务的完成回执迟到或用户关闭页面
- **THEN** 宿主 MUST 拒绝该回执恢复播放状态，新试听只能由用户再次明确启动
## ADDED Requirements

### Requirement: Generation completion evidence
The service generation core SHALL latch samples, SHA256 and terminal state only after model iteration and cleanup exit. Empty, cancelled, expired, oversized or failed generation SHALL NOT produce completed evidence. Completion evidence SHALL NOT grant playback or cache permission without independent transport and identity verification.

#### Scenario: Background cleanup follows successful generation
- **WHEN** ASGI cleanup requests cancellation after successful generation has been latched
- **THEN** the completed evidence remains unchanged

#### Scenario: Backend fails after yielding partial audio
- **WHEN** generation or cleanup fails after yielding PCM
- **THEN** the evidence reports failure and the partial audio is ineligible for successful cache publication

### Requirement: Matching HTTP generation completion
The Rust consumer SHALL require a same-origin completion receipt matching the stream response instance, request ID, actual received PCM sample count and SHA256 before reporting completion or issuing diagnostic Seal. Receipt reads SHALL remain bounded and cancellable within the original Operation deadline. Missing, expired, running, failed or mismatched receipts SHALL fail closed without automatically repeating synthesis.

#### Scenario: Normal HTTP EOF but failed generation
- **WHEN** the transport closes normally and the matching application receipt reports failed
- **THEN** the consumer rejects successful completion and cannot issue diagnostic Seal

#### Scenario: Completion query stalls
- **WHEN** the operator stops while waiting for the completion receipt
- **THEN** parent cancellation interrupts that query without waiting for the model or database

