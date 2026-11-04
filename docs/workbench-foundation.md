# V4.3 RF02基础运行入口

这是开发接口说明，普通用户工作台由RF03提供；不需要用户编辑这些技术参数。

## 实际装配

nai-foundation-engine（apps/workbench-engine第二个bin）装配nai-foundation-sqlite唯一writer与nai-llm-openai受信API/凭据adapter；策略位于nai-runtime-core，core不引用数据库或具体adapter。无媒体/旧runtime/旧Provider实现依赖，不创建额外云主机/Redis/NATS服务；只在显式生成时启动一个有界网络线程。

源契约：schemas/workbench/foundation.proto、schemas/config/foundation-root.schema.json、schemas/storage/foundation-v2.sql。Rust/TS/C++由cargo xtask schema生成，能力目录仅新增冻结workbench.foundation@1，通用产品/节目/交付路由仍draft。

## 入口和权限

内部受信父进程调用nai-foundation-engine --data-root <独立绝对路径> [--initialize]，通过环境注入临时NAI_WORKBENCH_TOKEN及认证绑定NAI_FOUNDATION_PRINCIPAL。不从请求正文取actor，不复用Provider Key或旧凭据scope。没有HTTP端点；不要拿任意客户端自报principal充当远程认证。RF03已接本地桌面常驻owner；远程/生产身份认证和Windows ACL仍留RF11。

RF03桌面固定使用`--desktop-owner`：空闲不触发旧演练4秒退出，仍有4秒请求期限/单槽队列/100万序号上限，EOF或壳退出回收。新用户运行`scripts/start-preparation-workbench.ps1`打开开发壳，工作区/草稿/节目/发布/私有预演均通过宿主访问，页面没有目录、主体、Key或原始SQL接口。开发程序不替换旧安装，详见[工作台报告](reports/2026-10-22-v43-rf03-preparation-workbench.md)。

RF04首个零API子任务增加`ask_faq/read_faq/resolve_faq`：只从当前有效的已发布FAQ取私有建议。任务/建议状态独立，5分钟以内有效，采用/编辑仅CAS追加话术草稿；结案和审计同事务。完整API任务预算/在途取消、语义检索和现场LiveContext尚未实现。普通用户在“发布与预演”选择已有问题即可查询，见[FAQ报告](reports/2026-10-22-v43-rf04-exact-faq.md)。

数据根首次必须显式初始化且为空；根清单+初始化journal可恢复初始崩溃，committed DB丢失不自动重建。后续打开独占owner.lock及SQLite writer；只通过命令读写。工作区Admin/Operator/Viewer分别管理角色/编写与交付/读取通用资料；私有卡正文另限recipient，不因workspace admin自动放宽。全局紧急停止仅受信root owner/父进程stop.request通道可触发，其他角色不获该全局权限。

命令覆盖创建工作区、授权/撤销、CAS对象写/读/墓碑、创建私有会话、准备/批准/提交私有交付、读交付历史和全局停止。此处通用对象不是已审核产品发布快照；不把仅结构和hash合法的人工文字解释为AI事实核验成功。

## 生命周期和回执

入口当前最多64请求/60秒、空闲4秒，操作/收发有界；这是基础开发组合，不用于持续直播。每次打开都会停止旧会话、递增epoch、撤掉旧批准。重新连接不恢复执行，必须新建显式会话。

DeliveryGate检查当前角色revision、session/epoch、对象版本/hash、范围/目标、单调期限、停止/撤销与准确bundle批准。准备队列最多16条，正文最多8192 UTF-8字节；媒体/公开目标缺能力直接拒绝。执行意图先记dispatching，私有卡投影后只记Submitted/Delivering；没有UI回执，不推定Completed或观众已读。进程强杀留下的dispatching为OutcomeUnknown，不自动重发。停止结果分别列cancelled_pending、already_submitted、dispatch_in_flight、outcome_unknown_count。

CNY预算接口先预留，再Known/Unknown/Released结算；未知不能释放掉，重启未结算预留保守转Unknown。真实用量、更多币种/价格与Provider任务对账留RF04，当前没有付费调用。

撤销意图先于可能失败的业务审计事务，重启仍拒绝未解决权限；扩大权限仅事务成功后生效。GC仅清理本writer有所有权标记的无引用正文；未知素材、内容不匹配的文件或标记保留，不承诺磁盘安全擦除/备份删除已完成。

## 定向验证

- cargo xtask check --profile workbench：RF01–04直接模块/消费者、依赖方向、注册表与前端。
- cargo xtask test --suite contracts：生成无漂移、Rust/TS/C++十五夹具互通。
- cargo test -p nai-foundation-sqlite -p nai-runtime-core --lib --locked：存储/策略故障与边界。
- cargo test -p nai-workbench-engine --test foundation --locked：真实进程保存、权限、交付、writer冲突、强杀、帧错误与输出堵塞。

测试全部创建临时根，无旧资产导入、安装替换或付费。详见[报告](reports/2026-10-22-v43-rf02-foundation.md)和[ADR0149](adr/0149-v43-foundation-owner-and-delivery-gate.md)。

## RF03草稿扩展

`put_preparation`使用FoundationObject字段（workspace_id/object_id/expected_revision/body），body须匹配`schemas/content/preparation.schema.json`。读取仍用read_object；不自动列出、不发布、不创建预演会话。正文及对象类型限制见[本轮报告](reports/2026-10-22-v43-rf03-preparation-drafts.md)。跨文档引用在此阶段仅验证形状，后续发布时检查闭包与事实有效性。

## RF03私有发布扩展

`publication_impact`绑定节目revision，生成30秒报告；`publish`须Admin提交report_id/hash及acknowledge_unknown，原子产生private/content_only引用快照。`read_publication`每次复检事实/撤销，返回PublicationStatus，不代表Delivery完成。`revoke_publication`以snapshot_id/hash定位错误产品版本，先持久化版本拒绝意图，再写撤销记录/审计，阻断相同产品版本的全部快照。

产物源Schema为`schemas/content/publication.schema.json`；不增加旧库兼容或表迁移。RF03已接普通用户表单与私有预演。发布边界见[发布报告](reports/2026-10-22-v43-rf03-publication.md)与[ADR0150](adr/0150-v43-private-publication-and-version-revocation.md)。

发布组合采用nai-foundation-v2根清单、schema_version=2、foundation-schema-2初始化标志及SQLite user_version=2。旧格式1开发根不自动打开或迁移，原文件保留；必须在新的独立空目录显式初始化格式2。旧格式SQL仅保留历史，不被当前root加载器执行。

## RF04短备稿与费用

`save_llm_model`/`set_llm_budget`仅Admin；读取设置不返回Key或密文。`start_llm`仅显式选择当前已发布单元、模型revision及本次预算上限，由owner先预留/记录派发意图再异步单次请求。`read_llm_task`/`cancel_llm`/`resolve_llm`仅任务创建者且当前仍有权限；采用只CAS追加草稿，未获得发布/播放批准。

Key密文保存于独立对象数据库，新Windows Credential Manager scope保存随机加密数据密钥；旧Key不导入。非Windows凭据后端未实现，明确Unavailable。已发送后取消/断线/超时/重启保留unknown占用，未发送才释放；不自动重试。当前单价为人工配置的估算，独立账单核对未实现。

UI入口：“模型与预算”保存模型/单价/预算 → “发布与预演”选择发布单元 → 勾选本次费用确认 → 生成 → 核对来源与限制 → 采用到草稿 → 保存场次最新引用并重新复核发布。真实付费供应商验收尚缺；完整边界和测试见[RF04报告](reports/2026-10-22-v43-rf04-bounded-llm.md)。

生成前只读本机检查会自动复核当前单元、模型、独立凭据和预算，显示预留估算与可用额度；本机通过不代表供应商认证或余额通过。检查不调用API、不写任务/预算/审计，实际生成仍重新检查。见[本机检查报告](reports/2026-10-23-v43-rf04-llm-local-check.md)。

模型设置页新增“测试连接与响应”：已保存模型+单次额度+明确费用确认，固定短句，无发布依赖。独立TestLlm任务沿用预算/outbox/单worker及取消结算，不生成可采用建议。普通文本响应测试与业务结构化备稿分别判断；见[实现与验证报告](reports/2026-10-23-v43-rf04-model-api-test.md)。

工作台导航已按使用频率分区：主界面仅产品/场次/预演；模型预算、主持系列、工作区管理及能力检查由右上角“设置”进入，返回恢复原工作页。首次创建使用默认工作区名称，不再用三步向导。见[布局报告](reports/2026-10-23-v43-workbench-settings-layout.md)。

模型设置现按LLM/TTS双Tab展示目录，各最多8项并支持新增、编辑、逻辑移除；Key留空保留，TTS使用独立凭据scope。共享预算折叠配置。TTS合成/试听未接入，详见[模型目录报告](reports/2026-10-23-v43-model-catalog-tabs.md)。

TTS执行前置模块已添加，但不是工作台可调用任务：仅内部Rust有界HTTP/PCM校验/取消，未接预算账本、音频产物或试听；详见[TTS前置报告](reports/2026-10-23-v43-tts-bounded-transport.md)。

TTS模型列表现有“测试合成”：固定短句、单次费用确认，TestTts/ReadTtsTask/CancelTts沿用唯一writer和累计预算，返回样本数/哈希，无PCM/文件路径；成功无用量证据仍unknown。正式试听尚未接入。见[任务报告](reports/2026-10-23-v43-tts-test-task.md)。

TTS测试音频现可在引擎RAM短期保留并主动释放：AuthorizeTtsAudio/RevokeTtsAudio仅返回/撤销描述符，重启无旧音频授权，任务和费用历史不丢。设备试听通道尚未接入。见[产物报告](reports/2026-10-23-v43-tts-audio-artifact.md)。

## RF05私有原生试听

设置 → 模型与预算 → TTS语音模型 → 测试合成成功 → 试听（本机）／停止试听。试听重用本次有效音频，不再调用Provider；缓存已过期或重启丢失时明确拒绝，重新合成仍须费用确认。使用系统默认音频设备；OBS采集桌面音频时可能录入试听。

StartTtsAudition/ RenewTtsAudition/ StopTtsAudition只控制测试音频，不能发布业务话术。桌面原生audition_nonce读取停止代次，start绑定它；workbench_stop_audition不获取owner锁，修改停止代次，独立worker每10ms检查。GUI续租600ms、worker挑战锚定租约<=500ms，迟到回复、重放、界面隐藏/离开/退出与失联不能恢复播放。回调只检查原子状态，无DB/模型锁。

nai-audition-worker仅按需启动，使用CPAL/WASAPI默认输出，无Python、Provider凭据或数据库接口；未新增常驻服务。临时PCM文件由引擎创建/回收，不接受网页路径；两个方向均有界Protobuf，控制帧上限4096B，不包含PCM。隔离进程并不等于OS沙箱或认证IPC；异常终止可能留下临时媒体文件，正式维护清理仍待实现。

Submitted仅表示样本已提交原生回调且等待有限尾部排空，不代表设备已播放完成。真实听感、端到端停止延迟、OBS路由、硬件ACK及公共业务媒体门控尚待验收。定向测试和限制见[试听报告](reports/2026-10-23-v43-native-audition.md)。

## RF05已发布本句的语音预演

发布与预演页先开始有效单元的私有文字预演，再在“本句语音预演”核对音色/字符估算/费用并批准合成，成功后点击“播放本句（本机）”。只有一个可用TTS模型时默认选择，多个可下拉；费用上限默认0.10元并折叠。没有可用模型时引导设置，不自动请求Provider或自动播放。

CheckPreviewSpeech只读，返回SpeechPlanView；SynthesizePreviewSpeech要求其approval_sha256，绑定唯一task、preview/source哈希、发布引用、session/epoch、actor/角色revision、文本、模型/音色revision、私有音频目标、TTL及单次费用上限。唯一writer重算匹配后原子持久化批准与预算/outbox；同一批准不能复制到新任务。

publication_tts_preview独立任务类型保存source引用而非任意输入文字，复用有界TTS/费用/取消/短缓存/独立原生播放。派发、迟到结果、音频授权及每次worker挑战重新验证source。StopPreview/撤销等完成后只清理失效缓存并停对应player，保留无关有效音频。停止预演先调用不取owner锁的原生停止；数据库失败仍受独立播放租约关闭约束。

任务完成只表示合成，不表示设备播完；声音Submitted仍非硬件ACK。不新增公共sink或自动讲品许可，也不推进节目检查点。测试与未完成边界见[报告](reports/2026-10-23-v43-published-speech.md)。

## 本次输出设备绑定

固定短句试听和本句业务语音均保留默认一键播放，新增“选择输出设备…”→选择现有输出设备→“使用此设备并播放”。列表在本次独立worker中生成，最多16项；系统默认设备优先，名称为显示用途，设备引用属于本次output_session_id。不是按设备名在另一进程重查，也不是持久硬件身份或跨重启自动路由配置。

SelectOutputDevice绑定workspace/task/output session/device ref；唯一可信root重新检查当前任务/source后，worker直接打开保存的实际设备对象。重复选择/旧session/未知ref拒绝；设备初始化后清掉旧lease许可并发新challenge，旧排队回复不能自授声音。选择阶段受GUI600ms/worker<=500ms租约、60秒选择期限及P0控制；控制与状态队列各2项，单帧4096B，媒体PCM仍单独传输。

设备失效或不支持时停止/Unavailable，不自动切另一设备；实际设备名称和私有监听用途可见。没有启动时后台枚举，不增加常驻服务；正常/错误退出回收设备对象。详情、真实测试及未认证边界见[报告](reports/2026-10-23-v43-output-device-binding.md)。
