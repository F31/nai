## Why
W06 已有真实RGBA生产者，W05/W06仍缺有界数据面槽位所有权与跨进程传输验证。依据V3.6 §26.3/31.2，先验证帧写入/提交/消费/释放、ABA与断连撤销，再接独立render-worker/keeper；不新增D编号。
## What Changes
- 增加provisional Protobuf槽位ticket/commit/event；metadata与控制通知分离于RGBA载荷。
- media-core增加唯一owner槽位状态机、大小/数量/期限与session/epoch/角色/代次复检。
- Linux显式tmpfs-file-drill通过/dev/shm私有文件及继承FD做实际子进程二进制读写，内容SHA256校验，输出owned帧；不引入unsafe或映射依赖，不标正式共享内存/设备Ready。
- 测试占用不可覆盖、重复/伪造ticket、释放后旧代次拒绝、截断/篡改、写入中强杀、超时与断连整池退休；新池需新identity，不复用旧内存。
## Capabilities
### New Capabilities
- `media-frame-slot-ownership`: 有界帧槽位所有权及工程数据面验证。
### Modified Capabilities
无；不冻结完整buffer.channel/output.deliver能力。
## Impact
schemas/domain/media.proto与生成产物、nai-media-core、跨语言契约fixture、ADR/文档/registry。复用prost/sha2/tempfile，增加直接依赖但无新外部包或常驻服务。上限16槽、8MiB每槽、64MiB每池；只有显式演练才创建Linux tmpfs资源，Windows后端不可用。
