# 设计
新增apps/public-output-keeper（media）作为新线公共待机宿主，固定窗口标题nai · 公共输出，静态自有NAI/STANDBY画面，不声称数字人资产或媒体播放ACK。仅Windows真实窗口；非Windows明确不可用，不用软件帧替代窗口证据。

控制契约追加独立PublicOutputLease/Status，4KiB有界继承stdio，窗口渲染线程不阻塞等待stdin。运行时复用挑战锚定LeaseGate算法：500ms上限、挑战过期/重复/旧代次拒绝，到期/输入EOF/优先停止后终态不可被迟到心跳复活。状态窗口帧计数只是实际待机画面刷新次数，不代表内容播放。许可始终不授业务输出，不能从健康心跳推导live_ready。

宿主使用固定二进制/随机临时token/有界I/O线程，关闭时独立回收进程，不阻塞数据库或模型；优先停止独立标记文件，以固定名称、绝对路径、无symlink/reparse及有界nonce复检，与私有audition.stop分离。UI每100ms控制轮询；WebView失联使租约到期并保持待机，显式关闭再重开新进程；shell正常退出回收，shell异常退出窗口仍处安全待机且用户可关闭。

本次keeper无媒体输入，无音轨初始化，固定帧不含私有提示/产品/Key，既有短租约只能用于宿主健康。未来公共bundle须另有session/epoch/审批hash/事实/租约门控，不能复用本次健康租约授播放。

验证：虚拟时钟终态/旧挑战/非法心跳；Windows真实独立进程/窗口/优先停止/EOF/partial stdin/强杀，实际更新计数与错误分开；相关依赖/生成漂移/原生IPC权限/前端构建。OBS真实录制及音画仍待后续；不新增无关全量测试。
