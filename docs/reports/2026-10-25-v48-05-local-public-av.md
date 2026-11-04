# V48-05 本地真实业务原文、公共2D与OBS选定组合验收

日期：2026-10-25。结论：本地Qwen→业务原文→发布/预演→公共批准→实际默认扬声器提交→2D/字幕→OBS应用音轨的单句工程链路取得完整实录证据。V48-05选定组合单句闭环完成；整卡长期稳定、全部设备/平台及产品ACC仍未整体通过，164ACC保持pending。

## 本次范围

组合：Windows、已保存qwen_local / qwen3-tts-1.7b-customvoice / serena / chinese、内置MIT几何2D、Speaker (Realtek(R) Audio)、Windows系统simhei字体、OBS33.0.0便携隔离副本。原文“容量为500毫升。”来自单独标注的演练商品规格；走完整发布和来源守门，不声称是真实在售商品或实际直播。

没有付费Provider调用、推流、桌面或真人麦克风录制。用户原资料、模型、Key、旧付费授权账本和原OBS安装配置保留。各调试目录保留，不自动删除验收工作区；每个任务一次派发，无自动重试。

## 解决的问题

1. 验收工具原来将等待引擎和待机窗口轮询串行化，真实结果落库可能饿死500ms健康心跳。补作用域独立待机轮询，不授予播放权限；1.1秒阻塞工作下实际窗口仍存活。
2. 固定追加60ms等待挤占执行租约余量，改为与桌面相同的100ms目标周期，扣除已执行耗时。
3. 同一调用重复校验来源导致250ms观察窗口余量不足。PublicCandidateSource在独占Store借用下保存已校验来源，guard消耗后归还Store；期间编译器禁止其他Store命令修改角色/事实/模型，取消、global fence代次、角色scope、文档闭包仍复检。每轮续租、每次权威read仍重新验证，不跨命令缓存授权。权威read内部去掉重复的第二次source读取，保留开头完整核验和末尾fence检查。
4. 开发构建对runtime-core/storage/jsonschema/serde_json定向opt-level=2，保持debug断言/溢出检查，250ms观察与500ms租约均不增大。原阶段guard完成约100–130ms，最终诊断约60–70ms；这不是完整性能认证。
5. 宿主续租拒绝先清理，再返回有限错误码，不再吞成空准备状态。最终移除临时阶段计时及stderr继承。

见ADR0174。无需新增Schema或迁移；新增类型仅为进程内所有权约束，未改变跨语言协议。

## 最终成功实测

证据目录：.nai-diagnostics/qwen-public-av-take11-2026-10-25。

- 工作区f5b5ba82-108c-49e2-9774-e148e175a4f6，任务e47b2e35-b811-4a29-970c-7460248879f9。
- 36480源样本全部提交，24kHz、约1.52秒；caption_observed=true。末状态submission_complete=true、playing=false、caption_visible=false、presentation已清除；不能将SDK提交称作硬件播放完成ACK。
- 原文批准bundle SHA256：b5ce995bfe81d57bf10f9cb10bab70a7e0dbde71cc780fa24041fd70817bbc8a。
- 字幕SHA256：273eacbf52f58af0f87d52bc1a5661017748ff465b293572a4d1b80cb548b518。
- OBS原录制：E:/projects/nai/.nai-diagnostics/qwen-public-av-2026-10-25/recordings/nai-qwen-av-2026-10-25-20-24-34.mkv。
- 60.1秒，H.264/512×288/30fps，AAC/48kHz/双声道。按实际帧PTS解码1801帧；字幕43帧、嘴型变化41帧。录像截帧人工视觉核对：原文可读，只有数字人画面，完成恢复STANDBY且无字幕。
- 字幕出现47.533秒、最后48.933秒；非静音音频47.610–49.010秒（10ms RMS阈值），峰值0.2495，整段RMS0.00753。字幕到非静音起点约77ms；视频33ms帧粒度，不能视为逐字口型同步或主观听感认证。前次按解码补帧计算约43ms，最终以实际PTS的77ms为准。
- OBS场景只有window_capture及scene，capture_audio=true；桌面/麦克风源为空。用户在自动输入冲突后手动确认开始录制；60秒OutputTimer自动停止封装。未操作“开始直播”。

可复核片段：.nai-diagnostics/qwen-public-av-take11-2026-10-25/public-av-preview.mp4；音轨recorded-speech.wav；speaking.png/completed.png；recording-analysis.json、timeline.jsonl、candidate-checks.jsonl及local-result.json。

## 验证与交付

- 本地验收模型选择边界1项、真实待机心跳1项通过；前者拒绝远程/带凭据/错误配置，不会自动选择付费模型。
- 宿主5项通过，包含有限续租拒绝码、未知码/畸形消息拒绝和原生私有字段边界。
- 存储116项通过，包含独占来源被取消后拒绝、角色/事实/会话/期限与批准撤销回归。受限运行曾全部IoFailed，原审批流程提升后同一组件全部通过，没有通过删测试解决。
- 最终Windows原生公共执行及五类故障矩阵2项通过；相关3包all-targets clippy干净。
- 引擎/keeper/桌面壳构建及GUI子系统/认证IPC启动通过。运行版更新，不改旧开始菜单安装。
- 未重跑完整workspace；本轮无Schema改动。
- OBS录制时包含显式开启的debug阶段计时；最终交付仅移除这些日志路径并再次通过原生故障矩阵。delivery-manifest.json记录最终二进制及录像哈希，不将录制前二进制冒称为最终哈希认证构建。

## 尚待完成

声音文字发音/主观听感由用户回放片段确认；SDK提交不替代物理扬声器完成证据。长句、长时间稳定性、多设备与真实抖音观众侧效果保持独立待验。没有将2D能力称为写实形象。

下一主线V48-06：统一LiveContext/任务取消树、中文知识检索引用和问题池；然后V48-07连续讲品、插问归位和真人独立音轨接管。不得把本次单句实录称为整个直播场景已完成。