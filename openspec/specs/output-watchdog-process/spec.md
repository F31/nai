# output-watchdog-process Specification

## Purpose
建立显式输出看门狗演练进程，验证继承控制通道在强杀、停续租和读写阻塞时不阻止独立租约撤权；保持工程诊断与真实音轨/窗口/播放ACK的边界，为W05/W10后续接线提供可复用机制。

## Requirements

### Requirement: Explicit unavailable drill process
输出进程 SHALL 仅显式watchdog-drill启动并只接收Rehearsal控制租约；设备状态 MUST 为Unavailable且output_available=false，不能产生真实播放完成ACK、授予正式Ready或以本轮证明R01/R03完整通过。

#### Scenario: Drill startup
- **WHEN** 演练进程新启动
- **THEN** 初始无租约，采用可信启动器明确指定的epoch floor，明确设备不可用；旧进程授权不恢复

### Requirement: Independent bounded lease watchdog
进程 SHALL 独立轮询MediaGate单调租约，不等待控制读、输出写或业务锁。输入队列最多16、帧最多64KiB、在途帧期限250ms；输出pending状态最多1份。EOF/非法帧/溢出/在途读取过期 MUST 关闭控制并撤权，迟到控制不得恢复。

#### Scenario: Producer killed or stalled
- **WHEN** 源进程强杀、停止续租或在partial frame中卡住
- **THEN** 软件租约失效并可观测，keeper控制诊断保持不可输出，不伪造硬件静音证据

### Requirement: Scoped monotonic drill commands
命令 SHALL 绑定启动session、单调seq和剩余budget（最多250ms），lease期限最多600ms；旧epoch、过期/旧seq、非演练模式及未知操作 MUST 拒绝。每条消费前须先复检租约。

#### Scenario: Late renewal
- **WHEN** 租约已失效后旧epoch续租或迟到命令到达
- **THEN** 不恢复租约，需新的更高epoch显式准入

### Requirement: Honest native-process evidence
测试 SHALL 真正启动和强杀自有子进程、有超时和清理，并分别记录软件观测与设备未测。控制源fixture MUST 只位于tests，正式engine/keeper/worker/bridge故障矩阵仍保留剩余缺口。

#### Scenario: Keeper restart
- **WHEN** 强杀keeper后启动新实例
- **THEN** 新进程无旧授权，测试不把重启当作窗口/声卡自动恢复通过

### Requirement: Keeper frame consumption with standby semantics

keeper SHALL 仅在有效租约下消费已提交帧；租约失效、epoch 陈旧、池退休或完整性失败 MUST 进入待机并停止投递。待机状态 MUST 通过状态消息可观测，且 `output_available` MUST 保持 false。

#### Scenario: Frame presented under valid lease
- **WHEN** 租约有效且收到通过校验的已提交帧
- **THEN** keeper 消费该帧并报告 presenting=true

#### Scenario: Lease lost switches to standby
- **WHEN** 已投递帧后租约到期或被撤销
- **THEN** keeper 停止投递、报告待机原因（LeaseLost）且不产生播放 ACK

#### Scenario: Retired pool switches to standby
- **WHEN** 收到 peer 错误或池已退休后仍有帧事件
- **THEN** keeper 保持待机并拒绝对应帧

### Requirement: Explicit frame-drill activation

帧路径 SHALL 仅在显式 `--frame-drill` 下启用；池文件通过继承的标准流失交付并由 keeper 只读重开，MUST NOT 通过控制总线传帧数据或 Base64。未启用时 keeper MUST 不建立任何帧路径。

#### Scenario: Drill not selected
- **WHEN** keeper 以普通 watchdog 模式启动
- **THEN** 不存在帧消费者，提交帧请求被拒绝且不产生 presenting 证据
