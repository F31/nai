# V561 流式语音原生试听/停止与统一取消

日期2026-11-07。C05a/b/c完成，C05整体仍未完成；应用EOS、身份缓存、FlashHead同源音画与OBS仍为C05d。旧安装/旧数据根/Key未修改，完整v561-start启动器仍拒绝未完成的直播装配。

## 新入口与控制链

新增nai-v561-voice-workbench原生GUI，沿用现有Tauri/React包，独立配置目录、窗口label、app identifier和仅4个native权限，不桥接历史workspace/foundation协议。它是必要输出安全/性能体验入口，不是第二套数字人内核或完整直播工作台。选择显式当前CosyVoice JSON配置后，输入测试文本即可试听；停止按钮不受普通状态轮询锁阻塞。页面隐藏/关闭会请求停止，不自动恢复或重试。

控制链：React小型Protobuf控制DTO→Tauri四个窄命令→nai-workbench-host单飞监督→私有stdio的nai-v561-audition-control→当前Tasks/OutputOwner和父子Operation→C04独立流式音频进程。PCM不经过网页/JSON控制总线。新增audition_control.proto先生成Rust/TS/C++/Python，再改消费者；现有302条消息、17个Protobuf源，生成无漂移。

C04播放函数新增audition_in_task消费外部Operation，其自身仅取消模型I/O子Operation，不提前撤销调用方任务。原生读线程收到Stop/EOF先调用weak StopHandle撤权，再入普通消息队列；旧完成复检当前task，取消后不能回报Completed。宿主停止只推进原子代次，监督线程独立发停止/回收子进程；新试听须等待旧进程结束，并由用户再次明确点击。

宿主输出和输入队列有界，stdio控制帧16KiB/回执4KiB；过期、错误版本/范围/非法状态不进入UI。停止后旧回执按代次剥离，配置只来自native选择或显式CLI，不让网页提交路径/执行许可。配置标签仅显示basename，完成状态分开报告源样本/设备消费样本。软件结束不是硬件静音确认；模型服务后台排空也不是GPU抢占取消。

## 实测与定向验证

4项Windows原生宿主测试通过（固定本机HTTP夹具、静音PCM）：正常完成返回设备样本；等待响应时优先停止且拒绝迟到完成、显式重开task2；关闭desktop owner取消HTTP连接（超时不能冒充关闭）；启动期间Stop不被迟到Start覆盖。另4项CosyVoice适配回归及1项桌面权限/资源边界测试通过。

computer-use通过实际原生窗口验证：配置已选、试听按钮可用、播放期间配置/文本/开始禁用而停止可用；两次完整试听显示3.36秒/3.20秒设备样本消费；长句生成过程中点击停止，后续快照显示“本次试听已停止，不会自动重播”，按钮恢复可再次开始，后续旧模型输出未将状态改回完成。最后重新打开最终构建，验证Ready态，不自动合成。没有人工听感或硬件静音确认。

真实本地请求共4次：服务漂移时1次格式拒绝，合并后2次完整试听及1次用户界面取消。都是明确的测试操作，没有自动模型重试。原生输入几何不可用时重新观察后重试点击，未产生额外模型调用；短句第二次完成时已来不及停止，如实保留，随后用长句验证取消，不将完成冒充停止。

新界面已在E盘target-windows/build-desktop/debug/nai-v561-voice-workbench.exe打开；辅助程序在target-windows/v561-owner-late/debug。开发启动脚本scripts/start-v561-voice.ps1，可显式传Config/RuntimeDir/DesktopBinary；不替换开始菜单旧版本。前端静态产物在target-windows/v561-voice-ui，复用E盘已有依赖，不重写node_modules或下载依赖。

## 原生验收发现并修复的问题

1. Tauri生成宏实际按配置父目录读取标准tauri.conf.json，最初自定义同目录文件名仍加载了旧静态页面。改为v561-voice/tauri.conf.json及显式构建变更追踪，实际URL已确认为v561-voice.html。初次窗口仅注册新四个命令，旧页面连接未获处理，没有打开旧数据库。错误尝试保留，不算界面通过。
2. 原生实测时发现CosyVoice容器已重建，当前入口新增/spks与zero_shot_spk_id，但丢失stream/格式头/单飞，模型也回到共享块长。客户端拒绝格式，未把整段输出冒充流式。先备份并严格核对实际hash，再用merge-cosyvoice-live-entry.py合并已验证辅助逻辑，保留全部新增音色分支。
3. 合并候选执行10项请求隔离/故障测试及6项接口测试通过，新增缓存音色免上传分支覆盖。重新创建API容器，保留GPU、端口、模型卷、代理环境、已有1个音色ID；不输出代理值，临时env文件用后删除。旧容器保留为cosyvoice3-api-before-v561-c05且手动停止，无数据删除。新入口、model.py、file_utils.py和nai_request_gate.py均只读挂载，模型卷维持原权限，重启继承挂载；Dockerfile补丁源也同步。手动重建仍须保留挂载，不能使用旧镜像/旧docker run参数省略它们。
4. 本次新建的C盘WebView profile已迁至E盘target-windows/build-desktop/debug/v561-voice-webview，原路径保留目录联接。最终GUI显式使用exe同目录的项目profile，不硬编码C盘。只处理本入口新建缓存，没有触碰其他应用或旧工作台数据。

服务合并/备份/activation.json、界面停止/Ready可访问性记录及前后服务日志位于target-windows/cosyvoice-voice-ui-validation。新API当前health=ok/busy=false；原生控制/音频进程均已回收，GUI空闲。没有推流、录音或付费调用。

## 检查与剩余范围

新前端定向typecheck/Vite构建、GUI/宿主/控制二进制构建与相关clippy -D warnings通过；schema --check及18包依赖检查通过。只运行相关测试，没有全量Rust/UI/GPU回归。OpenSpec整体保持活动，34ACC不升级。

下一张完整任务是C05d前半：供应商应用EOS/失败边界及模型/参考音色身份，完整缓存只在可核实结束后发布；随后将同一任务源样本时钟接FlashHead，完成实际连续音画与OBS。服务只做排空、未实现GPU硬取消，完整直播单实例/固定设备策略/主启动器/正式安装仍未验。
