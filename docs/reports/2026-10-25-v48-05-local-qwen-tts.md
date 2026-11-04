# V48-05 本地Qwen TTS配置与合成闭环

日期：2026-10-25；工程实现/定向实测已完成；V48-05整体仍InProgress，164ACC全部pending。

## 实现

先修改foundation.proto和publication JSON Schema，生成Rust/TS/C++及配置类型，再接入storage/root/transport/UI。独立qwen_local协议允许受限字面回环HTTP免Key，强制no_proxy、不携带Authorization、不重定向；远程服务保持原凭据边界。协议与language随共享目录版本保存，不伪造credential_saved。

常用服务增加“本地Qwen3-TTS”，默认http://127.0.0.1:8000/v1、qwen3-tts-1.7b-customvoice、serena、中文。本机地址/模型可展开编辑。音色为aiden、dylan、eric、ono_anna、ryan、serena、sohee、uncle_fu、vivian；11语言可选。列表基于用户服务实际/v1/voices及/v1/models响应快照，非运行时动态刷新。

用户关闭后，通过认证Owner和正常SaveTtsModel向原共享目录保存模型。未直接写SQLite，未覆写旧模型或Key；重读目录显示2个TTS模型。同地址/协议/模型重复时复用而非再建。

## 实测证据

- 策略3项、传输8项、存储116项、相关UI13项通过；前端类型检查/构建通过。
- 原生远程协议合成/已发布语音撤销回归2项通过，仅本地mock。第一次受限运行在Windows凭据环境报IO_FAILED，原审批流程提升后相同测试通过。
- Rust/TypeScript 70条fixture互通；229消息生成无漂移；相关all-targets clippy通过。
- 引擎/播放器/桌面壳构建通过；Windows PE GUI子系统及认证IPC启动检查通过（live_ready=false）。
- 保存后的真实本地API合成一次5251ms，59520样本、24kHz、2.48秒，缓存授权成功。音频SHA256为a0ab3a9c25c2404ffee56bfc1cbfd8304619e97fd97c204c0f5646d251cfa6c6；无自动播放、无付费API、无自动重试。诊断证据：.nai-diagnostics/qwen-local-registration-2026-10-25/local-test.json。
- 原生验证使用正常Owner入口，未启用NAI_LLM_TEST_LOOPBACK。独立运行版已更新，旧开始菜单安装不变。

## 边界与后续

9音色均通过选择/表单回读测试，真实合成仅serena，其他音色未逐一合成。真实生成/缓存不授予声音设备听感、公共守门播放、数字人音画或OBS录制通过。用户可在模型列表点测试合成后试听；外部Qwen服务须保持运行。本轮未新增推理常驻依赖或付费供应商调用。

下一步使用合法业务原文完成本地模型业务预演与公共音画，按已有守门、审批及租约流程补设备/OBS验收。
本轮OpenSpec已归档为2026-10-25-v48-05-local-qwen-tts，新主规格严格校验通过。全规格格式校验发现4项历史Purpose占位，未在本轮扩大修改；这不影响本轮编译/定向验证，但全规格校验尚非全绿。此前受限环境的Get-Process未列出启动进程，不能据此断定程序已退出；后续在提升权限的只读检查中核对窗口标题。
