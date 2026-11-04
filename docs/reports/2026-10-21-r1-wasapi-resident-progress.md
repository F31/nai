# r1-wasapi-resident-progress（2026-10-21）

## 交付

常驻本机输出生产路径已从CPAL预计时间改接安全WASAPI IAudioClock。每会话一个stream跨cue/静音待机复用；成功释放原生缓冲后登记cue绝对尾帧，逐buffer独立保存撤权/epoch/最早期限与计数；32条在途上限，拒绝早期buffer过期后被续租掩盖。编码后提交前再次拒绝撤权，空闲零样本不能冒领cue进度。

先修改media.proto再生成Rust/TS/C++；状态新增实际played可选帧数/观测标记/后端身份。宿主两个成功出口要求本次摘要、独立预期数、known backend和末次授权复检；operator保存playback.pb。桌面播放结果显示引擎结论“本机音频已到达设备播放尾部；音画完成仍需人工确认”。架构及成本见 [ADR0144](../adr/0144-wasapi-resident-progress.md)。

## 验证

- Windows最终Queue定向测试9/9：无write/部分位置、尾部不计零填充、双声道帧数、静音/epoch/期限/error、跨cue隔离、32条上限/早期限保留、编码后撤权；日志 `.nai-diagnostics/wasapi-native-final.log`。
- 宿主定向测试2/2：拒绝外来摘要、旧预计进度、缺失/未知后端、部分/超额/无观测及伪造完整A/V标志；Rust protobuf往返。
- Windows真实keeper常驻测试1/1：4秒192000帧与后续24000帧精确到达实际设备尾部，同一PID/stream，取消/换epoch/帧篡改/恢复/EOF待机；最终日志 `.nai-diagnostics/wasapi-native-keeper.log`。
- Windows真实engine+本地LLM/TTS夹具+设备1/1：预热零API、审批播放24000帧，核对实际进度及完整报告保存；再次播放复用keeper，途中P0返回STALE_EPOCH，无完整PlaybackAck；最终日志 `.nai-diagnostics/wasapi-native-engine.log`。
- TS契约descriptor4/4；contracts build与schema --check生成无漂移通过。
- 播放UI定向7/7；前端typecheck及相关Linux/Windows lint、格式收尾见最终记录。

没有全仓测试、付费API请求、安装替换或用户库/凭据修改。真实设备证据只覆盖本机默认设备和上述夹具，不升级registry认证。

## 未完成与下一步

此轮真正接通了生产候选的实际音频进度，但并未完成完整音画ACK和自动讲品。窗口仍按复制进度更新，未证明逐帧呈现/口型同步，也未实录OBS。节目继续人工确认，不写自动已完成句/检查点。下一步补齐音画完成判据与检查点，再连接有限节目连续讲品与插问归位，随后真人麦克风及OBS整场验收。设备切换/长播/正式IPC认证仍缺。

收尾：Windows相关三包all-targets clippy -D warnings、Linux相关三包lint、Rust格式与前端typecheck通过。

规格收尾显式移除两条旧callback预计时间成功判据，保留字段为诊断。归档重放首次检查发现两份既有历史归档未登记Git，导致三项主规格缺tracked出处；仅用git add -N登记该两份既有归档路径，不改其内容、不暂存内容，然后复核。

最终收尾：依赖方向20个workspace包通过；registry57/57、归档重放/提案8/8、主规格44/44、归档187/187通过。OpenSpec已归档为2026-10-21-r1-wasapi-resident-progress，验收项状态不变。

安装组合门槛补记：full首次发现新增lib.rs Windows模块入口未登记平台矩阵，补入既有approved-local-playback能力的源码/原生时钟测试路径；未更改平台支持状态或运行逻辑。

组合门槛第二次发现既有test:frontend-bundle脚本未登记处置，按其真实Chromium前置登记manual及文档命令，不删测试、不把未运行报告为通过。

文档检查续补：两份历史活动目录在HEAD中均无文件，仅残留flags=20004000的空intent-to-add，且实际文件已归档。首次git add -u被审批拦截；只读核实并添加HEAD为空/工作区文件不存在的前置保护后，获准清除空索引占位；没有暂存删除或改动实际文件。full已通过前置不重跑，从文档和编译步骤继续执行全部剩余等价门槛。
