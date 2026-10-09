## Context
V3.6 §3.3/§8.3/§12.1/§20.3/§31 W05/W10。MediaGate已有600ms短租约、单调期限、epoch floor和独立EmergencyHandle。

## Decisions
- 仅显式watchdog-drill可启动；只接受Rehearsal租约。设备/窗口未实现，状态带固定backend_error=CapabilityNotReady和output_available=false，绝不生成OutputAck或假播放记录。
- 新Protobuf工程子契约为provisional（ADR0015），先生成再实现；不将它冻结为完整output.deliver接口。session与重启epoch floor由可信启动器明确指定，不能重置floor来接受旧epoch；继承的stdio是私有通道，无网络监听。生产Windows对端身份/ACL命名管道未实施，不能将此管道视为平台认证。
- 控制reader线程使用既有64KiB二进制帧规则，记录首字节收到的单调时间；frame开始后250ms期限由主看门狗监督，避免同步stdin partial-read阻塞绕过deadline。空闲等首字节不是在途请求。
- 主线程每10ms复检门控与在途frame期限。输入队列最多16；溢出/EOF/格式错误关闭控制、撤权，保持演练待机。逻辑关闭后迟到读取结果不再入队；阻塞系统读取线程在进程退出时回收，不宣称取消了底层同步read。
- stdout由独立writer线程写入，pending状态只存最新一份。主线程try_lock替换状态，不等待输出锁；统计跳过的状态。状态仅是带观测时间的诊断，不能据旧状态授予播放。
- lease采用既有MediaGate::arm/renew，剩余期限最多600ms；seq单调、session固定、命令budget≤250ms，过期/旧seq/旧epoch不能恢复。timer在处理下一条前再次poll，不能被连续输入饿死。
- tests/support/lease_source.rs仅标准库：从测试生成的opaque frame文件向stdout写入后停续租/卡partial-read。测试调用项目rustc编译到tempdir，不加入产品包；通过ChildStdout直接接keeper stdin，父测试可真正kill源进程。
- 进程故障测试有超时与Drop清理，仅证明演练软件边界。engine/bridge/worker正式进程、Windows设备静音与画面故障行为仍无证据。测试报告记录强杀耗时，不用它标完整R03通过。

## Resources
1个显式进程、主/reader/writer三个线程；16×64KiB控制输入上限，1份pending状态（另1份正在写），不加载Provider/SDK/GPU。CPU/RSS设计预算未做正式长播测量，默认不常驻，不新增云服务。

## Verification
输入期限/seq/mode/session/epoch测试；真实源进程强杀、源存活停续租、partial frame卡读时到期、keeper强杀/新进程权限清空、非法帧拒绝。contracts suite实际编译与三语言互通新状态；portable与schema --check；registry/计划/报告准确记录部分完成。
