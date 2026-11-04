# V48-05 内置公共2D形象与实际许可观察

本轮工程范围完成，Windows原生定向验证、独立运行版更新及启动通过，已重新打开工作台。V48-05整体仍InProgress，164ACC保持pending。

## 实现

- media-core新增独立V4.8资源模块：原创几何绘图生成512×288 RGB888闭嘴/开嘴贴图，内存像素载荷两张合计1,179,648字节。逐行/逐块检查期限与取消，包SHA256覆盖格式版本、尺寸/帧率及实际像素。
- 作品独立随附assets/public-avatar/LICENSE.txt许可文本，嵌入二进制并计算完整文本SHA256。没有使用下载模型、人物肖像或测试fixture作为生产形象。
- root与独立keeper共享资源算法。候选准备检查重算实际资源hash，可以将形象/许可轴标为passed；实际公共guard和业务输出仍not_ready，approval_available=false。
- keeper显示默认形象的闭嘴静态待机画面，状态携带实际资源/许可元数据；宿主和UI拒绝缺失或未知元数据。公共窗口仍standby=true、live_ready=false、audio_active=false，健康租约不授予业务消费权。
- UI默认使用内置资源，无需额外导入或选择；候选页与公共输出页显示形象名、尺寸和许可标识。

## 实测

- 像素/许可确定性、内容损坏与取消2项通过。
- 待机租约4项通过。
- Windows实际窗口及优先停止、进程强杀、控制失联、partial输入/EOF/坏帧3项通过，核对实际形象元数据。
- 原生引擎本地HTTP fixture整链1项通过：实际PCM与形象资源核验、公共批准仍拒绝、陈旧hash及事实撤权拒绝。不是付费Provider认证。
- 相关前端11项通过，前端类型检查及生产构建通过。
- Rust→TS→Rust 45条消息互通通过；212条Protobuf消息及相关生成无漂移通过。
- 相关5包all-targets clippy及27包分层依赖检查通过。
- 引擎、keeper和桌面壳构建通过，桌面隔离smoke返回authenticated owner protocol=2、live_ready=false；确认更新前相关进程均未运行，已重新打开独立工作台。

本轮未执行全量测试、真实付费API或OBS实录，未修改用户模型、Key或业务资料，未更新旧开始菜单安装。

更新后只读确认工作台及foundation引擎持续运行。OpenSpec严格校验通过并归档为2026-10-24-v48-05-builtin-public-avatar。

## 工程处理与边界

Cargo.lock首次原位写入因Windows映射文件返回1224：保留完整副本后以相同内容原子替换，再由Cargo更新，仅新增两个内部nai-media-core依赖条目。窗口测试首次链接遇到LNK1104，既有helper重试后3项通过。

根LICENSE为Apache文本、Cargo标识为MIT的既有差异未在本轮处理；本作品使用独立明确许可，不以该差异推断其他文件许可，不声称完成全仓许可审计。

30fps仅为资源及窗口的目标声明，未作为实测设备帧率。像素逻辑及窗口/故障已实测，但人工外观确认与OBS录制未执行。当前开嘴贴图只准备，不随业务音频驱动；自定义形象导入/许可管理、生产公共guard、公共批准UI、业务PCM/帧通道和播放完成证据仍未完成。

下一主线仍V48-05：将当前本句、事实来源、实际音画资源和输出身份装配公共guard，再接明确批准及keeper业务媒体消费，最后做语音/口型、接管与OBS实录验收。参见ADR0160。
