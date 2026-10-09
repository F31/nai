# ADR 0017：帧槽位所有权与Linux tmpfs-file演练边界

- 日期：2026-10-09
- 状态：Accepted
- 基线：V3.6 §26.3/31.2 W05/W06；不改变Windows正式共享内存目标。

## 决策
先交付有界、单owner槽位状态机与实际跨进程二进制传输验证。保持workspace unsafe_code=forbid，不为前置验证引入未经审查的内存映射/平台SDK依赖。
Linux工程后端使用/dev/shm私有tmpfs文件、0600权限、unlink与继承FD、safe Rust分块FileExt读写；名为tmpfs-file-drill，有复制，不称为mmap/零拷贝或正式buffer.channel支持。
仅显式--shared-frame-drill执行，测试/演练素材不混入生产形象包。Windows/非Linux后端明确CapabilityNotReady。

## 所有权与撤销
Free→Writing→Ready→Reading→Free；root ticket绑定整份metadata、双方身份、epoch、pool identity和slot_generation。
未release的consumer槽不可复用。超时/断连/内容损坏退休整池；新identity与新内存，不复用旧writer仍可能访问的资源。
逻辑退休不保证恶意/卡死peer立即关闭FD；supervisor须kill/wait回收进程。root读为独立owned副本，哈希只验完整性，输出仍须MediaGate/keeper复检授权/审批/事实/租约。

## 资源与部署
最多16槽、8MiB/槽、64MiB/池；P0不持槽位锁、不等待帧IO。没有新常驻进程、云依赖或生产profile必选服务。
复用已锁定prost/sha2/tempfile。Windows映射、protected handle broker、命名管道身份、render-worker/keeper生产装配、真实性能与设备认证仍待后续；本ADR不替代这些验收。
