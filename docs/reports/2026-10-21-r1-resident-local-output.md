# R1 常驻本机输出窗口（2026-10-21）

## 本轮结果

实现并实测会话内 keeper/PID/minifb 窗口复用，首次显式播放建窗，句间黑色静音待机。两个不同已批准候选使用同一输出进程；每句仍走真实 operator TTS/2D/审批/事实/epoch/租约路径。单句上限20秒，帧64MiB；不完整截断不报成功。

新增私有 LocalOutputCommand（描述符、grant、cancel、shutdown），媒体仍走文件；生成器为grant分支装箱，不手改生成代码。主控队列2、keeper输入4、状态2、一个在途播放。取消只撤权，不授予后续播放；主控750ms内确认待机则保留窗口，失败回收进程。流析构前所有早退先撤回调授权；EOF/失租独立拒绝。UI生成/播放忙碌时人工接管与立即静音均可点。

承接 ADR0134，新增 [ADR0137](../adr/0137-resident-local-output.md)。registry A01/R03只补证据，不升级状态。没有调用付费API、更新安装版、迁移用户数据库或提交已有改动。

## 定向验证

- Windows `cargo test -p nai-engine -p nai-output-keeper --lib --offline`：13+14 passed。
- Windows keeper `--test local_playback -- --ignored --test-threads=1 --nocapture`：2 passed。真实WASAPI/minifb/子进程；测试音频与授权为明确fixture。双句各24000个48k样本提交；同PID待机、主动取消、新epoch新句、EOF退出；篡改和停止续租拒绝。未生成硬件PlaybackAck。
- Windows engine `--test engine approved_local_playback -- --ignored --test-threads=1 --nocapture`：1 passed，内含成功/撤权两个场景。两个不同候选仅spawn一个keeper；静音后收到待机确认，provider未被自动再次调用；回收临时文件且无playback_ack日志。
- Linux engine operator模块：20 passed，覆盖Provider/审批/预览/知识热发布的直接回归。
- Linux engine+keeper clippy all-targets `-D warnings`通过；Windows相同范围clippy通过（文件系统硬链接缓存警告不影响编译）。最终边界新增用例由Windows lib运行。
- TS contracts build、desktop typecheck、preview/workbench UI：12 passed，包括播放中人工接管可用。
- `cargo xtask schema --check`：Rust/TS/C++/JSON Schema/OpenAPI无漂移。契约suite通过：Rust 8、Node 39，C++编译与两条三语言互通通过；随后仅对新增命令四种分支复验互通，结果见收尾补记。

原生设备测试须有Windows音频服务访问权限；受限沙箱内回调停滞/析构等待的失败已保留诊断，等价测试在实际设备环境通过。Windows既有链接锁LNK1104通过同命令重试消除。集成夹具发现accepted socket在Windows继承nonblocking导致WouldBlock，现显式设置blocking并保留10秒读取期限；修正后原生和20项operator回归通过。不是重试付费调用。

## 真实边界与下一步

整个直播场景仍未完成。当前音频stream逐句创建，常驻最多30分钟；待机窗口黑色，尚无中性形象待机/长播认证。提交样本和window update是软件观测，actual_local_playback_completed保持false。暂无连续讲品生产调度/插问归位UI、真实麦克风独立轨、OBS实录；这些未实现项保持未开始，不写成外部Blocked。详见 [执行卡](../r1-delivery-plan.md)。

下一完整切片先补合法中性待机、有界帧流与稳定音频stream生命周期（20秒与64MiB双限同时生效，正常分辨率可能先触帧预算），再将PitchRunner接入生产讲品/插问归位，随后真实单路麦克风、回声与显式恢复，最后OBS实际录制验收；不直接推流。不用本轮两句fixture播放替代这些用户场景。

## 收尾补记

新增命令的descriptor、grant、cancel及shutdown=false四分支完成Rust→TS→C++→Rust、Rust→C++→TS→Rust复验：23份fixture、20种消息，64位整数/中文路径/oneof false存在性保持。仅复跑互通步骤，未重复39项结构检查。registry检查57项通过；OpenSpec已归档为2026-10-21-r1-resident-local-output。

归档后新主规格严格校验通过，历史归档178项全部通过；已补齐归档器生成的Purpose占位。
