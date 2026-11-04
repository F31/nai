# V48-05公共待机窗口与独立守护链路（2026-10-24）

## 本轮交付与真实范围

新增按需nai-public-output-keeper：实际Windows GUI独立窗口，固定标题“nai · 公共输出”，显示仓库自有MIT几何NAI/STANDBY画面。无人物素材、私有提词、产品信息、PCM或业务媒体输入；不是数字人渲染完成。界面新增公共输出页及打开/停止保留待机/关闭按钮，已打开时右上角提供不受业务忙碌或未保存锁阻塞的停止入口。

独立Protobuf PublicOutputLease/Status从Schema生成，复用4KiB有界帧编码和挑战锚定LeaseGate算法；每进程随机token通过继承环境与私有stdio匹配，页面不能指定路径/token/媒体。健康租约上限500ms，过期挑战/旧序号/错误token/非法帧拒绝；输入半包读线程不阻塞窗口与租约轮询。超时、EOF或优先停止后为终态，旧心跳不恢复；需关闭再显式重开。

停止使用独立public-output.stop标记，路径/符号链接/reparse/nonce大小校验，与audition.stop分开，优先操作不获取数据库或模型锁。原生I/O在独立有界线程及spawn_blocking内处理，窗口只由keeper绘制，前端轮询不承载媒体。宿主正常关闭回收子进程，控制失联保持待机，用户可关闭窗口。健康状态恒live_ready=false/audio_active=false，不授予播放许可。

## 已实测

- 4项runtime-core租约状态测试通过，包含停止/到期不可复活、错误token/挑战/序号/期限拒绝、迟到不按到达时间续期、健康不授媒体权限。
- 3项Windows真实窗口/进程测试通过，覆盖窗口创建及实际待机刷新、独立停止、keeper强杀检测、控制失联、半包、EOF、超长非法帧。测试窗口置屏幕外，终态窗口继续存在且保持待机；这是窗口API证据，不是屏幕视觉质量或OBS录制证据。
- 共用辅助函数的既有私有音频停止回归1项通过，公共停止不创建audition.stop。
- 前端/相邻源契约与CI测试13项通过，状态解码拒绝伪造live_ready/audio_active及不一致终态；前端类型/生产构建通过。
- 相关clippy无警告、27包依赖检查、206条Protobuf及配置生成无漂移通过；新keeper已加入workbench检查和Windows CI原生构建/测试入口，未声称远端CI已运行。
- 桌面壳与keeper构建、独立原生启动smoke通过；实际keeper PE子系统=2，后台启动无控制台窗口。测试后无keeper残留进程。

修正过程中的Copy clone、范围判断clippy警告后复验通过；未运行无关全量测试、未调用付费API、未读取或输出Key。

## 运行版与GUI验证

检测工作台已退出后完成更新，scripts/start-preparation-workbench.ps1现定位并检查新keeper，运行版已打开。使用computer-use技能确认实际WebView的公共输出菜单已呈现；自动点击先遇到coordinate input geometry is unavailable，再遇到user input was detected，刷新后窗口已最小化，因此停止继续抢占窗口。打开/停止/关闭的WebView实际点击闭环仍待人工确认，不能写成GUI操作已通过。原生宿主调用和真实窗口故障测试已完成。

未自动打开用户的公共窗口、未改业务数据库/模型/Key。用户可进入公共输出页，明确点击打开公共待机窗口；OBS窗口捕获可选该标题，但实际OBS捕获和录制尚未验收。

## 资源与后续

新增1个按需媒体进程，复用已锁定minifb 0.28.0，无云/端口/音频/模型服务依赖；自有512×288×4像素缓存589824字节，目标窗口刷新30FPS，控制轮询100ms，队列容量有界。实际RSS/CPU和长时负载尚未基准，不以缓存大小当总内存。未启用时不启动该进程，关闭后回收。见ADR0156。

V48-05整体保持InProgress，164ACC全部pending。后续依次补齐：公共媒体bundle的session/epoch/审批hash/事实/资产及租约门控；独立媒体数据面和正式音轨/音频时钟；合法2D资产与worker渲染/字幕；Windows强杀/设备故障及OBS实际录制。以上未实现，不冒充外部Blocked，也不提前切换至V48-06。
