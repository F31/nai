# ADR 0007：会话撤销与独立媒体门控

- 日期：2026-10-07
- 状态：Accepted
- 基线：V3.5 第 3.5、21 章；D06

## 决策

kernel Session 管理生命周期/模式/epoch，授权 actor 与 preflight 来自可信 composition root。
媒体的 EmergencyHandle 单独原子撤销计数，不获取内核/模型/数据库锁。
调用顺序为媒体撤销 → session epoch 更新 → 存储检查点 → 丢弃迟到任务；存储失败不会恢复输出。
句级 TaskToken 与会话 epoch 两层取消，不承诺强杀线程或抢占 GPU。

media-core 保留独占有界元数据队列，每次消费与 poll 检查 P0/本地单调期限。
首版租约上限 600ms，可按约 200ms 续租；续租不改授权集合，也不能复活已经失效的租约。
解除静音必须显式授予更高 epoch 的新租约。引擎恢复用已存 epoch 与存活媒体 epoch floor 的最大值 +1；
仅恢复 Stopped/Standby/Assisted，不恢复绑定、审批或自动权限。

规范化候选哈希原算法移至 contracts，storage 重导出，复用已锁定 sha2，不改变
sha256-canonical-v1 与已存哈希。生成 BTreeMap 使 Protobuf map 编码顺序稳定；媒体重算对象
并复检审批、事实/策略、模式/值守、守门与权限。contracts 只提供结构算法，不作运行授权判断。

Session 内命令去重上限 256，单条指纹 ≤4096 字节，重试记录最长 300 秒；容量满时拒绝新命令，
不淘汰窗口内记录。P0 专用静音信号在普通命令 admission 前执行。重试窗口由 root 配置，不能超过
上述上限；恢复清空内存去重记录后，旧 epoch/revision 命令失效，不自动重播。

storage checkpoint 只持久化撤销 epoch/revision，同事务撤销审批、取消未完成 utterances、记录审计。
它不是全会话状态保存接口，运行生命周期存储接线需后续 composition root 完成。

## 范围与验收

Linux/WSL 状态机、门控、锁阻塞/续租中断、篡改对象、CAS 与 ACK 测试通过。
此轮只交付 portable 门控；真实媒体工作循环、PCM/设备缓冲、独立进程 IPC 与 Windows 热键
尚未实现（D11），不把原子信号测试当作实际音轨静音或 P95 测量。
Windows kernel/media GNU 检查通过；storage GNU 检查缺 MinGW C 编译器，原生设备验收无环境。
无常驻服务、云资源、新模型或付费 API。
