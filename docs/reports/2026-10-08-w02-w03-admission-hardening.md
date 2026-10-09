# W02/W03 接管审查与完整性修复

- 日期：2026-10-08；环境：Linux/WSL，项目锁定工具链。
- 基线：V3.6 §5.8/§21.4，工作项 W02/W03；验收前置 R07/S01/S09。
- 变更：`w02-w03-admission-hardening`；没有新增 D 编号。

## 接管审查结论

复核 Kimi 交付的 decision.rs、0005 迁移、duty.rs、Session、测试及归档主规格：哈希链、STRICT/拒改删、分级缓冲与值守期限机制可复用。Schema 5和备份生成契约继续有效。

发现并修复：
1. 主规格要求同事务冲刷，原实现却每条独立事务，后续失败可留下部分提交。
2. 原 append 忽略 flush 失败后继续写新遥测，造成新记录越过缓冲头。
3. 原 critical 也先尝试整个缓冲冲刷，消耗关键提交的预算。
4. 原 Mute/Takeover 只撤 epoch，不清有效值守；Running/AutoSupervised 下自动回复仍可能许可。
5. 值守撤权计数耗尽失败后重新确认，可在旧自动模式下重新准入。

保留上一轮归档与历史报告，新规格描述修复语义，不重写已落盘哈希或迁移。

## 实现

- 遥测缓冲与新条目整批同事务、按序重新链接，任一错误完整回滚；成功提交后才清内存缓冲。显式 flush 同样原子。
- critical 独立事务，不先冲刷遥测；其失败仍原样返回且不入缓冲。序号表示提交顺序，critical可先于降级遥测，created_at保留原记录时间。
- 成功静音/接管/停止清除值守并退出自动模式；非自动模式保留。重新确认值守不隐式切回自动，仍需明确模式操作。
- enforce_duty 计数耗尽时退出自动，保留失败返回，不谎报已完成epoch撤权。Valid枚举注释修正为非许可标记。
- 无 schema/API/hash格式变更、无新增依赖、进程或常驻成本。缓冲仍1024条/单条64KiB，原子事务保留op检查。

## 实测

- 先补回归，旧实现实测出现4项storage失败及1项kernel耗尽失败；随后修复。新增静音/接管/停止回归同时覆盖幂等和明确恢复条件。
- `cargo test -p nai-storage -p nai-kernel -p nai-control-api --offline`：通过，storage44项、kernel65项、control8项。
- `cargo xtask check --profile portable`：通过，12包边界、registry、fmt、workspace Clippy、全 workspace **238项Rust测试**（232+6）。
- `cargo xtask schema --check`：生成无漂移，备份契约仍schema5。
- 新增6项：遥测不超车、冲刷失败全回滚、缓冲+新遥测同事务、critical不先冲刷、控制撤权清值守、epoch/revision耗尽拒恢复。
- 本轮未重跑完整C++互通、前端或Windows认证；以上不得代替设备性能/正式直播验收。
- OpenSpec实施后 strict validate、archive、主规格及归档校验通过：19个主规格、17项归档；本轮归档为 `openspec/changes/archive/2026-10-08-w02-w03-admission-hardening/`，5/5任务完成。

## 保留边界与后续任务

- R07仍为implemented：桩Provider回放一致性、真实播放ACK与日志生产接线待W17/runtime。
- `enforce_duty(now)`没有自身定时线程，不能单凭其测试证明600ms实际停声；可信root须独立调度、先撤销媒体并处理持久化/计数失败。
- output-keeper/render-worker尚未实现；强杀engine/keeper/worker/bridge测试没有执行，R03不标通过。
- 下一主线W05：独立媒体租约、紧急通道、窗口/待机/标识与设备适配；W06共享内存/2D与W10故障演练依赖其推进。Windows设备证据另行验收。
- 哈希链是逻辑一致性机制，没有外部锚定，不能证明拥有数据库写权限者未整体改写或截断；不作为许可/身份认证替代。
