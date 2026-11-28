# 隔离OBS失败提前停止闭环

## 结果与范围

L06e5完成。此前验收失败退出后，隐藏OBS仍等待1830秒定时器，本轮增加专用本地停止路径。无模型调用、无GPU推理、不录桌面或麦克风、不推流；没有更新桌面二进制、配置数据库或Key。L06整体及34ACC/M0不变，完整30分钟混合验收尚需重跑。

## 实现

`scripts/start-v561-isolated-recording.ps1`仍拒绝任意既有脚本，在核对专用profile/collection、输出目录与无既有进程后，复制仓库固定Lua到本次新artifact目录，自动绑定随机32字符nonce。每次保留scene修改前快照，仅改变本次专用collection。新`obs-control.json`与原`obs.ready`记录进程、日志和文件身份；控制标记已存在则拒绝重放。此信号用于隔离运行身份，不宣称抵御同用户恶意修改本地文件。

Lua仅在真实录制开始后有界读取stop.request（最多33字节），精确匹配nonce时调用正常停止，实际RECORDING_STOPPED后才写stop.receipt；不能开启录制、推流或执行命令。`scripts/stop-v561-isolated-recording.ps1`核对项目路径/进程与启动时间、同nonce回执、实际Recording Stop日志及非空文件，再写obs.stopped。超时保留进程/录像，不强杀。定时器仍作最后兜底。

启动器新增可选`-TestProcessId`：在ready后持续监测本次验收进程，遇到进程退出、mixed-complete.json或录制定时器上限，在finally中请求停止。因此可以覆盖harness断言失败，也能在原生测试等待obs.stopped前完成正常封口。使用此参数时应让启动器作为独立编排进程运行，原生测试进程并行运行；不传参数则只握手并返回，由调用方显式停止。若握手前启动失败，仍保留定时器兜底，不虚报停止成功。

采用OBS官方[前端API](https://docs.obsproject.com/reference-frontend-api#c.obs_frontend_recording_stop)的正常停止及实际停止事件；脚本装配字段依据[官方scripts-tool源码](https://github.com/obsproject/obs-studio/blob/32.0.1/frontend/plugins/frontend-tools/scripts.cpp)。不启用WebSocket或新增服务/网络端口。不修改用户D盘OBS。

## 验证

- 8项PowerShell边界：合法配置、麦克风拒绝、定时器缺失拒绝、任意脚本拒绝、旧stop标记拒绝、旧ready拒绝、缺准备标记拒绝、项目外路径拒绝。全部通过，无真实进程。
- 真实隔离短录制一：`target-windows/obs-stop-native/589baff30f5d4b9baee3b8003e03ddfc`，ready41ms；错误nonce等待2秒未停止，正确nonce正常停止，重复停止幂等。85帧完整解码成功，音轨2.794秒。
- 真实隔离短录制二：`target-windows/obs-stop-native/fa534bea687747928368e7a029dbd2cd`，ready134ms；无模型测试进程等待10秒后以失败码7退出，监控自动停止，重复请求幂等。61帧完整解码成功，音轨2.005秒；前段为OBS自身启动时间，录像早于45秒兜底定时器结束。
- 两次均保留原录像/日志/回执，实际正常封口及完整解码后才清理对应闲置隔离OBS进程。录制只有空的指定公共捕获目标，不当作实际人物音画证据。
- OpenSpec strict、V5.6.1注册表检查通过，34项仍pending。CodeGraph按需查询未覆盖当前PowerShell/Lua路径，已直接核对源码与定向测试；未运行无关全量构建。

## 后续

使用现有默认512输出token的实时模型配置，接入上述监测参数，重新执行三问题分布的30分钟真实混合/OBS。必须核对整段覆盖、gap长尾、回答/恢复、停止及实际录像；本轮不授予该项通过，保留11月18日截断失败。
