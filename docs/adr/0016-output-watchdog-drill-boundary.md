# ADR 0016：输出看门狗演练与正式设备边界

- 日期：2026-10-08；状态：Accepted（工程演练范围）。
- 基线：V3.6 §3.3、§8、§12、W05/W10；命名nai-*。

先实现显式output-keeper watchdog-drill进程，复用MediaGate验证独立租约与进程失联，而不是把进程内单测当成完整故障隔离。首轮通过可信启动器继承stdio，无公网/本机网络监听，临时工程子契约为provisional。

启动器必须明确给出session与已知epoch floor；MediaGate增加无授权的floor构造入口，重启拒绝该floor及以下旧租约。不推测持久化/幸存媒体floor，正式supervisor须从可信恢复材料核对。

该进程拒绝正式模式，始终报告真实设备Unavailable，没有PCM/帧消费或PlaybackAck。它不修改Local默认受保护IPC、不安装服务、不替代正式独立P0通道。下一轮窗口/音频/热键与命名管道身份校验须继续按W05任务验收，不能将工程stdio当作Windows权限认证。

控制同步read由独立线程执行，主看门狗监督在途期限并逻辑关闭，进程退出回收阻塞线程；writer不持有看门狗锁执行IO。16条输入、64KiB帧、1份pending诊断状态限制资源；无额外第三方运行依赖。维护成本为显式进程/三线程，RSS/CPU与长播另行测量。

真实强杀只对tests自有opaque-frame源进程和演练keeper进行，报告明确不等于真实engine/render-worker/SDK故障矩阵或实际600ms停声。正式输出只有在设备、审批/事实/守门与媒体通道接线完成后开放。
