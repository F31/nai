# runtime-composition 变更（w70）

## MODIFIED Requirements

### Requirement: Shared session between control surface and composition

控制面（HTTP 命令处理）与装配根（租约签发、值守、决策日志、故障处置）SHALL 共享同一会话实例
（`Arc<Mutex<Session>>`），使撤权产生的 epoch 递增对两侧立即可见。查询与命令路径的锁竞争 MUST 以显式
冲突错误处理，且 MUST NOT 阻塞或回退已执行的媒体静音。**撤权与故障处置**路径上的瞬态竞争 SHALL 在有界
预算内重试，耗尽 MUST 显式上报，MUST NOT 静默跳过撤权。**轮询读取**（epoch 活性检查）MUST NOT 把竞争
当故障：SHALL 在有界预算内重试，耗尽报「本 tick 未观测到」，MUST NOT 报错或退出。所有预算 MUST ≤ 30s。

#### Scenario: HTTP revoke visible to supervisor

- **WHEN** 控制面执行静音/接管使会话撤权
- **THEN** 装配根随后签发的租约与记录的决策日志使用递增后的 epoch；keeper 持有的旧 epoch 租约失效

#### Scenario: Session lock busy

- **WHEN** 会话锁被占用时到达控制或故障处置
- **THEN** 返回显式冲突错误；已执行的 P0 静音不回退

#### Scenario: Transient contention on the revoke path

- **WHEN** 会话锁被一次微秒级、自我消除的竞争短暂占用，而撤权或故障处置正在执行
- **THEN** 该动作在有界预算内重试并最终落地；MUST NOT 因这次竞争被跳过，也 MUST NOT 被报告为终局失败

#### Scenario: Contention during the epoch poll

- **WHEN** 活性检查读取会话锁时遇到同样的瞬态竞争，且竞争持续超过其预算
- **THEN** 该 tick 报告「未观测到」并继续；MUST NOT 被当作故障，MUST NOT 终止监督循环或触发撤权

### Requirement: Engine supervision loop

引擎在配置 keeper 时 SHALL 运行监督循环：spawn keeper、以共享会话 epoch 签发 Rehearsal 租约并 arm、
按配置间隔续租、驱动值守 tick 与故障 observe。控制面撤权后引擎 MAY 显式重新签发租约（记决策日志）；
子进程故障后 MUST NOT 自动重启输出。监督循环 MUST NOT 因瞬态锁竞争而终止：活性检查失败 SHALL 视为
「本 tick 未观测到变化」并继续执行故障 observe；循环的终止只允许来自 HALT、REVOKED 或真实故障。

#### Scenario: Keeper killed under engine supervision

- **WHEN** 被引擎监督的 keeper 被强杀
- **THEN** 引擎静音、撤权、写 critical 决策日志并转待机；控制服务保持可达；输出不自动恢复

#### Scenario: Renewal stops after a control revoke

- **WHEN** 控制面撤权使 epoch 递增且 keeper 仍被监督
- **THEN** 引擎停止续租（记录 `renewal_halted_after_revoke`）并让既有租约到期失效；
  重新授予输出权限 MUST 由显式操作者决定，引擎 MUST NOT 自动重新签发

#### Scenario: Renewal failure does not silently exit

- **WHEN** 续租失败（例如子进程已消失）
- **THEN** 引擎记录失败并继续执行故障处置（静音→撤权→critical 日志→待机），
  MUST NOT 在未撤权的情况下退出监督循环

#### Scenario: Transient lock on the liveness read

- **WHEN** 活性检查因瞬态锁竞争读不到 epoch
- **THEN** 引擎跳过本 tick 的撤权判定，继续执行续租与故障 observe；监督循环 MUST NOT 结束，
  且后续 tick SHALL 恢复该判定

## ADDED Requirements

### Requirement: Bring-up operations are budgeted for the work they include

包含 schema 迁移的一次性 bring-up（`Store::open`、可信 owner `--provision`）SHALL 使用**具名的 bring-up
预算**，MUST NOT 与逐次调用的小预算共用取值；bring-up 预算 MUST 大于逐次调用预算，且 MUST ≤ ADR 0004 的
30s 上界（ADR 0038 的安全层不变量）。不含迁移的逐次调用 SHALL 保持小预算。bring-up 失败 MUST 指名
**步骤**与**已耗时**，MUST NOT 只报一个期限错误码。

#### Scenario: Fresh store with the full migration chain

- **WHEN** 在空数据根上执行 bring-up，需要跑完全部迁移脚本
- **THEN** 该操作在 bring-up 预算内完成；预算取值 MUST NOT 小于同仓同类迁移操作的既有取值

#### Scenario: Bring-up fails

- **WHEN** bring-up 的某一步失败，含预算耗尽
- **THEN** 错误指名该步骤与已耗时，使该次失败可判定，而不是只给出一个期限错误码
