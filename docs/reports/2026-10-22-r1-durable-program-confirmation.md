# 2026-10-22 节目人工复核持久化

## 交付

关闭“听完确认只存在内存、重启没有线索”的子缺口。节目确认先复核本轮精确本机候选的播放报告SHA256、独立预期设备帧数、IAudioClock实际尾部和窗口提交诊断，再复检实时批准对象/事实、有效期、epoch、快照与节目期限。报告只按引擎生成的文件名读≤16KiB，缺失、替换、错误epoch、未知/不完整进度拒绝。

人工复核通过现有critical cue哈希链日志写入数据库，固定event=`program_human_confirmation`，持久成功后才推进。记录操作者、节目/段落/候选、媒体与报告摘要、实际设备帧与窗口提交诊断，不保存未播草稿。写入失败不前进；提交后撤权/冲突保留审计但暂停。相同命令幂等，不自动重试或额外调用模型。

引擎读取当前工作区最近256条审计中最新确认，返回链引用；重启节目仍absent，仅显示近期历史，不恢复runner、候选、审批、预算或自动播放。桌面以默认折叠的详情展示，无新增必填或确认步骤。OpenAPI先新增ProgramConfirmation及可选last_confirmation，再完整生成契约，无手改生成代码；没有数据库迁移、新依赖、第二存储owner或常驻服务。

设计边界见 [ADR0145](../adr/0145-durable-human-program-confirmation.md)。**这仍是人工复核记录，不是机器音画完成、已完成句、场次记忆或可自动恢复的正式检查点。**

## 实测

| 范围 | 结果 |
|---|---|
| 引擎节目lib | 6/6，含写入失败不前进、提交后撤权暂停且保留记录、只前进一次、报告替换/缺失/超限/不完整/错误epoch拒绝 |
| 存储近期记录 | 1/1，critical持久、重开读取、工作区隔离、忽略同tag遥测、256条窗口、哈希链验证 |
| 真实引擎重启 | WSL 1/1、Windows 1/1，本地测试记录显式种入审计；重启absent、历史只读且无LLM/TTS请求，不冒充设备证据 |
| 既有有限节目集成 | 最终3/3，预算、未知事实、预览不能确认、单次生成/重放/静音回归 |
| Windows真实设备+引擎 | 1/1，明确本地协议Provider fixture；实际本机播放24000设备帧，替换报告拒绝、恢复后确认、同命令重放仅一条critical记录，关机后库内记录及哈希链有效，无PlaybackAck |
| 桌面相关 | 24/24，含严格人工记录解析、历史不恢复、实际组件折叠记录展示及既有节目/播放/原生IPC；typecheck、生产build通过 |
| 契约 | schema正负样本39/39；生成无漂移通过（最终结果见schema-check-final日志） |
| 静态 | fmt；engine/storage/control-api all-target clippy -D warnings通过；Windows引擎及工程身份桌面壳实际构建通过 |

仅执行改动与直接消费者检查，无全仓测试或真实付费API调用。Windows测试是实际设备接线，但人工确认由测试命令提交，不是人耳/OBS验收。机器完整音画完成、正式检查点、连续自动讲品/插问归位、真人麦克风、OBS录制仍未完成，registry状态不升级。

## 失败与修正记录

- 最初选错`--bin`测试目标，运行0项，不计通过；改用`--lib`后6项实际执行通过。
- 最后修改接口说明导致生成的http.ts注释漂移；从源完整重生成后无漂移与typecheck通过，未手改生成文件。
- Windows首轮测试链接LNK1104，未发现同名活动测试进程；同命令重试链接并实测通过，不修改断言。
- 既有三项节目集成首轮并发时，阻塞LLM夹具在5秒等待内未收到请求（2通过/1超时）；无代理、顺序复核3/3通过，未放宽断言或新增业务重试。首次超时的确切根因未证明，不能据复跑宣称已经修复。

证据在`.nai-diagnostics/program-confirmation-{test,storage,restart,windows-restart,integration-recheck,device-recheck,ui,contracts,clippy,schema-check-final,shell-build}.log`。OpenSpec已归档为`2026-10-22-r1-durable-program-confirmation`，合并后的basic-selling-loop主规格严格校验通过。

## 安装状态

用户确认保存退出后，已维护更新原目录`D:\nai Engineering`的桌面壳、引擎与输出keeper，三文件均与已验证暂存清单SHA256一致。完整旧安装与用户目录备份在`target-windows/installation-backups/before-program-confirmation-20261022-061548`。更新前后8个用户文件数量/哈希一致，凭据绑定身份不变，无付费API调用。

安装版GUI subsystem=2，`--smoke`退出0且报告`SHELL_UI_READY`。结果见`.nai-diagnostics/program-confirmation-install-result.json`；仅验证实际界面挂载，未替用户生成/播放或提交人工确认。此为工程版定向维护更新，旧NSIS安装包未重新发布。维护脚本首轮清单比较因PowerShell运算优先级拒绝，发生在任何文件修改前，补明确括号后完整更新成功。
