# 应用优化构建落地与首次启动分阶段复验

日期：2026-11-16。工程切片完成，正式34ACC仍pending、M0 No-Go。

## 实际结果

native-05真实本地TTS+FlashHead768/4准备、四句两次重播、停止、新epoch恢复及改正文拒绝，159.16秒测试通过。5次本地合成（预热1+正文4），重播0调用；未推流、未录OBS。首声上界2560/675ms；六个device gap204/187/171/188/190/203ms；两轮538/538源帧提交窗口。停止62ms仅软件回执，未授硬件静音期限。独立assess-v561-prepared-plan评估功能和短程预算均通过。

首次音频会话总1984.462ms，内部1983.094ms，其中spawn858.043ms、pipe53.504ms、设备OpenScript1070.476ms、renew0.894ms；第二次337.644ms，其中spawn19.630ms、pipe39.245ms、设备278.218ms。首次前置等待基本由已测会话启动解释。但native-04的6714ms失败无当时分阶段数据，本轮没有复现，不宣称修复其全部根因或长期首声稳定通过。

运行前物理6916016KiB（约6.60GiB）、可用提交19565548KiB（约18.66GiB），TTS健康空闲，无重复Windows模型实例。运行前资源不等于整轮采样。TTS预热首PCM1957ms、完整2643ms。与前轮音频长度不同，不能用帧总量变化推断吞吐；本轮只按实际帧身份和设备时钟评估。

## 源码交付

补全audio_session_startup的父等待和spawn/pipe/OpenScript/renew聚合计时，本地报告O(1)不传媒体、不改变协议。原有像素转换/窗口计时保留。

按OpenSpec v561-playback-optimized-build严格校验后，Cargo常规dev的nai-workbench-engine设opt-level=2，保留debug断言、溢出检查和既有全部媒体复检。普通cargo build无临时覆盖后exe SHA256与native-05完全一致：CC4C14F93720BD4612FDCD258BC3A41B8A942ED55581CFF28A0D30D851EF570E。真实实测组合由此与常规构建一致，无需再调用模型验证同一二进制。

颜色1、时钟/取消2、缓存9共12项定向单元测试通过；真实Windows整链1项；应用lib/bin clippy -D warnings通过。编译器缓存硬链接失败自动复制的提示保留，非Clippy诊断。普通构建、OpenSpec及注册表检查执行；不做全workspace重复回归。

未覆盖体验版或安装程序。旧runtime及native-01至04失败全部保留，未将短程通过覆盖长稳失败；30分钟性能与独占OBS继续在prepared-narration-cache中保持未完成。本工程切片归档后，下一项为优化组合30分钟复验及首次启动分布观察，再接AI插问/归位。

证据：target-windows/v561-playback-profile-review/native-05/native-summary.json、assessment.json、play-3/script.json、play-4/script.json及phase-fingerprint.json。
