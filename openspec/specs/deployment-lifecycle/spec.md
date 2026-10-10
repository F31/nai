# deployment-lifecycle Specification

## Purpose
把「部署与档位」的另一半从声明变成可执行判据：**未启用/未安装的能力 MUST 不占任何进程、端口、显存或云请求**；空闲资源 MUST 按 TTL 释放，MUST NOT 提前释放（提前释放等于制造冷启动）也 MUST NOT 永不释放；档位升级 MUST 通过加装包与配置迁移完成且 MUST NOT 重装内核、MUST NOT 要求先装中间档；离线安装 MUST 在缺件时**拒绝**而不是抓取；部署形态迁移 MUST 在维护窗口内进行、MUST 保证权限与证据版本与数据哈希一致、输出切换 MUST 不重复播报。本 capability 不授予真实 GPU 配额探测、真实轻云实例或真实设备长播的认证。

## Requirements

### Requirement: Disabled capability holds no resource

能力 MUST 以**声明式足迹**（进程、端口、显存、云请求四族）描述其占用，且足迹 MUST 在**启用时**才物化。已安装但未启用的能力、以及未安装的能力 MUST 不持有任何资源。启用时若某**排他**资源（进程名、端口）已被另一启用能力持有，MUST 拒绝而不是覆盖。

#### Scenario: Install without enabling holds nothing
- **WHEN** 安装一个声明了进程、端口、显存与云请求的能力，但不启用它
- **THEN** 账本中该能力持有为空，且审计不报告该能力占用任何资源

#### Scenario: Exclusive resource conflict is refused
- **WHEN** 启用一个声明了已被另一启用能力持有的端口的能力
- **THEN** 启用被拒绝，两个能力持有不变

#### Scenario: Audit reports a disabled holder
- **WHEN** 账本中出现「未启用却持有资源」的状态
- **THEN** 审计以稳定标记报告该违规，而不是静默通过

### Requirement: Idle resources are released by TTL, not immediately

场次结束后资源 MUST 保留至其 TTL 到期才释放；TTL MUST 分族设定（可重载代价不同）。`reclaim` MUST 只释放 TTL 已到期的声明，MUST 如实报告仍保留的声明数量。正在服务会话的能力 MUST NOT 被回收。

#### Scenario: Ending a session does not unload a model
- **WHEN** 某能力的最后一个会话结束，且其模型 TTL 尚未到期
- **THEN** 该能力仍持有显存声明，`reclaim` 报告其为保留项

#### Scenario: Reclaim after TTL releases the family
- **WHEN** 空闲时长超过某资源族的 TTL
- **THEN** 该族的声明被释放，账本不再持有它们

#### Scenario: Busy capability is never reclaimed
- **WHEN** 一个能力仍在服务至少一个会话
- **THEN** 它的任何资源都不被回收

#### Scenario: Reclaim never deletes data
- **WHEN** 空闲回收执行
- **THEN** 它只释放资源声明，不改变能力的启用状态，不删除任何存储数据

### Requirement: Tier change uses one migration graph

档位（Lite、Standard、Pro）之间的迁移 MUST 由**加装/移除能力集合**表达，MUST NOT 重装内核，MUST NOT 要求先安装中间档；跨档位（如 Lite → Pro）MUST 等价于沿途差集的并集。档位差异 MUST 来自启用能力，MUST NOT 来自档位名称本身。

#### Scenario: Lite to Pro skips the intermediate tier
- **WHEN** 计算 Lite → Pro 的迁移计划
- **THEN** 加装集合等于 Standard 与 Pro 两段增量之并，且不需要先到 Standard

#### Scenario: Kernel is never in the plan
- **WHEN** 计算任意两档位之间的迁移计划
- **THEN** 加装与移除集合都不含内核

#### Scenario: Same tier is a no-op
- **WHEN** 起点与目标档位相同
- **THEN** 加装与移除集合均为空

### Requirement: Offline install refuses instead of fetching

离线安装 MUST 在**每一个声明文件都在本地**时才继续。任一文件不在本地 MUST 显式拒绝并指出该文件，MUST NOT 回退到网络获取。离线安装 MUST 复用同一套校验（Schema、内容集合、逐文件 SHA256、签名、撤销）。

#### Scenario: Missing local artifact is refused
- **WHEN** 清单声明的某个文件不在本地工件集合中
- **THEN** 安装以明确的拒绝结束，不产生暂存，也不尝试获取

#### Scenario: Complete local set installs through the same verification
- **WHEN** 所有声明文件都在本地
- **THEN** 安装通过与在线路径相同的校验链，且签名或撤销不通过时同样拒绝

### Requirement: Deployment migration runs in a maintenance window with a single writer

部署形态迁移（Local → Hybrid → Distributed）MUST 在**无活动会话**时进行（实时链路留在原部署）。目标为分布式形态时 MUST NOT 允许同一 SQLite 存储被多节点同时写入。起点与目标相同 MUST 拒绝而不是空操作成功。

#### Scenario: Live session blocks migration
- **WHEN** 存在活动会话时规划迁移
- **THEN** 规划被拒绝，原部署继续运行

#### Scenario: Multi-writer sqlite is refused
- **WHEN** 目标为分布式形态且声明的 SQLite 写者多于一个
- **THEN** 规划被拒绝

### Requirement: Migration preserves permissions, evidence version and data hash

迁移 MUST 显式映射权限；任一权限在目标形态**无映射**时 MUST 拒绝，MUST NOT 静默丢弃。迁移后 MUST 复验**权限集合一致、证据版本一致、数据哈希一致**；任一不一致 MUST 拒绝。证据版本不一致 MUST 与哈希不一致可区分。

#### Scenario: Unmapped permission is refused
- **WHEN** 某个源权限在目标部署形态没有映射
- **THEN** 迁移被拒绝，权限不被丢弃也不被悄悄改写

#### Scenario: Evidence version mismatch is refused
- **WHEN** 导入后的证据版本与源不同
- **THEN** 复验失败，且失败原因是版本不一致而非哈希不一致

#### Scenario: Tampered import is refused
- **WHEN** 导入后的数据哈希与源不同
- **THEN** 复验失败

### Requirement: Output switch announces once

输出切换 MUST 幂等：同一切换键重放（例如维护窗口内的重试）MUST NOT 重复播报，第二次 MUST 报告为重复而非再次宣告。

#### Scenario: Retried switch does not re-announce
- **WHEN** 以同一个键执行输出切换两次
- **THEN** 第一次报告已播报，第二次报告为重复
