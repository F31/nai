## Context
复用W06 FrameMetadata/ownedRGBA及W05独立P0/租约。Rust unsafe_code=forbid，仓库没有映射依赖；生产Windows映射/IPC尚未实现。
## Decisions
- unique owner持有Free→Writing→Ready→Reading→Free状态。每次reserve递增slot_generation，ticket绑定pool/session/epoch/producer/consumer/整份metadata；slot_index只是偏移索引，不是权限。
- metadata根侧确定，commit不得改frame/长度；严格RGBA8 width×height×4、非负PTS/正duration/timebase、显式buffer_ref由owner产生。SHA256只校验传输完整性，不授权播放。
- reserve期限由owner单调钟固定，最多250ms；commit/read/release/validate均复检，不允许peer延长。任何活跃槽到期或声明端断连则整池永久Retired；不回收给旧writer仍能访问的内存。新池必须新identity；旧consumer拿到owned副本仍须keeper复检，不能视为播放授权。
- 上限16槽、8MiB/槽与64MiB/池；只允许固定声明producer/consumer，ids限128字节。release前不得覆盖Reading；commit/release重复或ABA返回StaleEpoch，禁止隐式重试推进。
- Linux tmpfs-file-drill：private目录位于真实/dev/shm tmpfs、属当前用户、mode0700；root以tempfile建0600文件/验证device与owner后unlink。FD继承仅演练root持有，不通过URL/path从control body打开资源。安全Rust FileExt分块read_at/write_at，有复制，不声明mmap/零拷贝。
- 根侧仅使用继承或自己创建的FD，子进程从/proc/self/fd/0接收数据FD；Protobuf ticket以有限hex CLI bootstrap（只有metadata，无PCM/RGBA），stdout只输出有界Protobuf事件。必须--shared-frame-drill显式启动；fixture像素只在该演练中生成，不是假生产avatar。
- 截断/读取/哈希/末次期限校验失败退休整池；reader得到独立Vec后仍持有Reading票据，release后才能复用槽。退休移除逻辑授权，实际内存直到所有FD关闭才释放，supervisor必须kill/wait残留peer。
- 与真实设备/MediaGate/keeper帧消费分开，P0现有路径不取槽位锁。ADR0017记录该后端边界；不把tmpfs-file称为正式Windows shared-memory能力。
## Verification
portable槽位回归；Linux真实子进程FD传输/partial强杀/临时文件权限/完整性/超时；跨语言新增消息双向互通；portable/contracts/schema/registry/OpenSpec与前端检查。A01/R03仍implemented。
