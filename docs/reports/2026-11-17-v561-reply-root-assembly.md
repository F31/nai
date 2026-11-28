# 已有回答文本插播与按句归位：root装配

## 实现范围

本轮把此前独立的单问题槽和游标接入v561_live_control，而不是继续做缓存基准。新增私有ReplyText命令，包含明确的问题与已有回答文本；宿主reply_text方法只允许在当前已备讲解活动时入一个待处理任务，不允许嵌套或覆盖。它没有调用LLM，也没有新Tauri/界面按钮，不称自动AI问答完成。

先改live_session.proto、正式生成后改消费者。326条消息生成并schema --check无漂移；原控制命令禁止夹带reply，插播命令禁止另夹text或缺少问题/回答，编码帧上限8192字节继续执行。现有Tauri窄命令边界不扩大。

## 实际装配

- Reader收到插播时增加控制generation，立即撤销原播放Operation；不是将插播伪装成用户Stop。用户Stop/Shutdown仍独立优先，并清除恢复意图。
- Plan identity绑定整段正文SHA与有序资产身份；内容发布代次保留不随插播变化。root持有Cursor和Slot，不从UI或日志文件恢复游标。
- v561_prepared_plan::play仅在已完成且设备实际消费样本与资产匹配时推进Cursor；中断句不前移。恢复入口同时检查计划身份、句数、游标位置、全部资产key/TTL，设备段数/报告样本总量改为剩余句，已完成句不从头播放。
- 独立v561_reply_playback协调器沿用同一PreparedSession：单TTS生成/播放已有回答，完整结束后复检Slot并恢复剩余原稿。回答与恢复共用有界Operation，非阻塞许可谓词同时检查generation、worker和计划TTL；停止、失联、过期或错误均丢弃意图，不重试。
- 停止与身份键计算之间的竞争通过结果分支处理，取消不会因bind失败直接把正常停止变成会话异常退出。

没有新服务、模型实例或依赖，没有降画质、扩大缓存或放宽deadline。窗口仍采用现有执行方式；本轮没有真实录像，不授窗口衔接或OBS可见性通过。

## 检查结果

- 调度内核7项、计划/游标4项、宿主3项、契约5项，共19项匹配Rust检查通过。
- TS/Python问题codec与相邻控制codec/配置验证，共3项Node通过。
- engine/host cargo check、engine lib+v561_live_control构建、原生v561_reply_native构建通过。
- engine/host lib及原生验收target clippy -D warnings通过；首次检查发现旧测试Request初始化漏reply字段，补齐后5项契约通过。一次clippy命令重复--lib已修正，未掩盖失败。
- 326消息正式生成无漂移、OpenSpec strict、34ACC注册表通过。C++只生成，不声称编译/四语言运行认证。
- 没有跑无关全量测试。硬链接缓存回退复制警告保留。

## Windows原生验证：尚未执行

独立组合target-windows/v561-reply-resume-review已准备runtime、profile.json、input.json、reply-native-test.exe、fingerprint.json和run-native.ps1。脚本只允本机健康空闲TTS、至少16GiB可用提交空间、新产物目录；失败不自动重试。

实际验收将确认：首句设备样本已消费→第二句期间插播→真实回答音画→同worker、新epoch、零额外生成恢复第二句；然后第二次插播等待中Stop，保持Stopped且没有迟到恢复。输入明确为已有回答，不用此探针授LLM通过。最多5次本地TTS（预热1、两句基线2、完整回答1、取消任务最多1）。

本轮CosyVoice健康ok/busy=false；先后可用提交约13.26、12.35、14.08GiB，未满足既有约16GiB加载前提。已向用户请求释放资源，未代关程序/重启WSL/更改电源；没有启动GPU验收，因此实际本轮LLM/TTS调用均0。不能把原生程序已构建说成整链已通过，L04b真实验证仍待补。

下一步先执行该独立组合的两条真实路径；通过后再接L05真实LLM入口及L06持续混合负载/人工等待体验/OBS。运行版/开始菜单未替换，34ACC仍pending，M0不升级，活动规格不归档。
