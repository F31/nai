# ADR0165：单句内容批准与执行许可分离

日期：2026-10-24。状态：采纳，V48-05工程增量。

原批准账本通过媒体执行准入检查来确认内容，错误要求输出设备已就绪。新增仅返回unit的内容审查接口，检查完整bundle、可信来源/角色/事实、实际音频与形象、许可、守门及观察时限，但不创建PublicMediaPermit。设备能力检查仍是播放准入的必需条件；生产观察明确output_supported=false，不虚构Ready。

root最多保存一份15秒单次草稿，完整bundle/hash与已核验原文一起展示；用户明确确认本机公共窗口使用权后，重新获取原生单次250ms窗口观察并复检当前资源/来源/守门。确认成功或失败均取走草稿，重复、过期或变更要求重新检查。保持原观察截止时间和停止nonce，不在数据库后重签；批准与审计原子提交，重启沿用撤权，撤销先围栏后写日志。未知直播平台权限仍不授予。

大型嵌套回执在Windows调试原生进程中暴露栈溢出。通过正式prost生成配置将FoundationResult.public_candidate_check、FoundationResult.public_approval、PublicCandidateCheck.approval_draft置于堆边界，保持Protobuf线格式及控制帧上限；不增加线程栈、不手改生成代码。原生进程回归覆盖此路径。

本轮没有播放入口。已有批准不能使待机窗口自动消费PCM/帧，公共业务音画、设备完成与OBS仍未实现，V48-05和164ACC不升级。
