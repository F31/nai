# CreatorBox 技术方案核查与 nai 借鉴建议

## 审查范围与限制

2026-10-30 只读审查用户提供的 `E:\projects\CreatorBox`，检出的 HEAD 为 `68a107a5aaa5ca7dce100886f184c42a70d617aa`，`pyproject.toml` 标识 1.0.32。本轮不运行其程序、不调用外网或模型、不修改该目录。核心 `interface/model.py`、`interface_streaming_v2.py`、`utils/features.py`、`utils/renderer.py`、`src/router/sockets.py`、`face/landmark_face/models/DINet.py` 均为 PyArmor 包装，正文不可直接审查；前端 JS 也经过混淆。以下区分可见文件、其文档描述和对 nai 的建议，不据被保护文件名推断算法实现、时序可靠性或许可。

## 可确认的设计与启示

1. 素材及控制入口：`static/templates/extend/dh.html` 将人物视频、文件/麦克风音频、OBS/cv2/ffplay 预览和启动/中断/清空/发送集中呈现；nai 应借鉴以「形象、声音、开始、停止/接管」为主线的交互，默认自动检测输出，细节放设置。表单支持不等于实际设备已验收。
2. 分层控制：`static/templates/common/socketio.html` 描述 OBS 状态和场景、数字人启动停止、发音频/中断及房间连接等 27 个事件。数字人启动参数包括 `video_path`、`camera_id`、`preview_model`，发音频参数为 `audio_path`；此接口可做独立进程适配可行性调研。事件列表是文档，不足以证明服务身份认证、访问控制、帧/音频传输、取消可靠性或消息回执语义。
3. 视频驱动和调度：本地 `docs/zh/component/dh.html` 描述视频及麦克风输入、实时打断/插播；日志展示 WeNet 音频特征预计算、指定帧率、音画差和跳帧统计。借鉴「分段特征缓存、预热、统一时间基准、可观察丢帧与音画偏差、优先插播」，不能以其日志数值或 WeNet 名称推断本机性能或完整模型组成。nai 已有样本计数、独立输出和句边界归位，应在其上接写实渲染。
4. 可选输出：其文档声称 OBS 虚拟摄像头输出，`pyproject.toml` 的 GPU 依赖包含 `pyvirtualcam` 和 `obs-websocket-py`。可借鉴以明确能力状态区分窗口预览、OBS 控制连接、虚拟摄像头、音轨及真实信号；虚拟摄像头只输出画面，音频仍需单独路由。虚拟摄像头组件兼容、可靠性及授权尚需实测。
5. 问题来源：`static/templates/extend/msg.html` 展示抖音/YouTube 链接输入、聊天/关注等过滤和回复；接口文档列 `connect_room` 和 `generate_reply`。连接方式、平台授权、事件真实性与稳定性由于核心受保护均未核实，不能据 UI 声称官方或合规接入。nai 应保持 platform/room/connection generation 绑定、筛选和有界问题池，并实测目标平台的正式授权途径。
6. 自动化能力边界：本地数字人文档同时写自动回复计划中、暂不支持实时生成文案；事件目录又列 `generate_reply`。应以锁定版本的实际测试明确手动触发与自动工作流，而不能认为已具备自主连续讲解。对 nai，复用已有问题池、场次及恢复能力，继续以真正的目标/资料/已播回执构建智能体决策。

## 模型与依赖观察

`face/landmark_face/checkpoints/test/opt.txt` 是可读配置，其中出现 `pirender_3dmm_mouth_hd` 和 `dinet_v1_20240131.pth`；`wenet` 目录和文档日志显示 WeNet 音频特征相关线索。实现正文受保护、实际权重未在该 checkpoints 目录发现，因此仅能判断存在相关配置与路径，不能证明运行时实际模型及其输入输出，更不能断言该版本完全基于 DINet、MuseTalk 或 ONNX。`pyproject.toml` 声明 Python、Torch、ONNX Runtime、FastAPI/Gradio、Socket.IO、OBS 和模型扩展依赖；依赖声明不代表每项功能已使用或可用。

本地根目录未发现明确的项目 LICENSE 文件；PyArmor 文件头中的 trial/non-profits 标识属于工具生成标记，不等于 CreatorBox 项目或模型的授权条款。使用源码、打包分发、模型权重或商用之前必须向权利人核实具体许可证；保持个人隔离调研与正式集成区分，不解密或直接复制受保护代码。各扩展子模块也有独立许可。

## 推荐执行顺序

1. 先检查 CreatorBox 可执行包/本地实例及授权，冻结版本并获取权重来源、哈希和许可；在不改变 nai 数据、系统代理和直播场景的隔离环境里做真实对照。固定同一视频、Qwen 音频和 RTX 5090，记录启动/首帧、画质、帧率、丢帧、音画同步、显存、30 分钟持续、插播恢复和 OBS 音视频；未经验证的平台连接保持未知。
2. 若服务有明确使用许可，做一次不推流的 Socket.IO 适配可行性验证：发现/状态、启动视频、提交已获授权音频、完成与中断、退出/失联；核对权限和路径边界，确认完成回执是否代表实际播放。由 nai 保持唯一业务策略、任务取消、媒体授权和明确用户控制。
3. 根据实测结果，在独立 OpenSpec/ADR 中评估外部本地服务 profile 的成本与限制。不得把 Python/Torch 渲染器标为 V5.2 NativeOnnx；保留本地 Qwen 和不同后端共存及 GPU 资源竞争检查。若接口、许可或实时能力不满足，保留现有 MuseTalk 研究路线，使用对照结果指导优化。

本报告为静态审查，不证明 CreatorBox 本机可运行、写实效果、性能或任一平台能力；也未改变 nai 代码、数据库、注册表和用户配置。
