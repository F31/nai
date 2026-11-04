## Design
live_run对象id同时为其独立session id，创建同事务建会话。单对象有界32单元、版本CAS；源上下文由当前发布闭包计算，不接受页面伪造水位。LiveContext返回源身份、run/session/epoch、控制人、游标、当前/下一单元、完成/跳过计数。

record_public_execution是root内部事件入口，按bundle session直接定位live_run（不存在则保持既有单句路径）。从bundle.audio.artifact_id读取TTS及父preview，验证cue、source、session/epoch及当前批准hash。开始记录在租约交付前；完成只消费已由keeper验证的提交完成事件，不声称硬件播放ACK。父暂停请求继续当前句，到回执才进入paused；继续不重发当前句。

重启使session停用/epoch递增。Read投影interrupted且不会派发；显式recover使用新epoch恢复到paused，无确认的在途句保持interrupted，只有明确skip才能前进（计为跳过）。end停止会话，旧lease不能续期；不是P0替代。跨角色、事实变化或错误session拒绝播出，允许结束清理。未知中断不标已完成。

UI复用当前语音准备与逐句批准，不添加假的自动播出。最近run id仅作本机便利索引，权威读取来自引擎。切换后清除旧卡；准备下一句须显式按钮。后续再提供批量审批/预取和跨句无缝输出，不把本轮称自动连续直播。

备播首页复用队列卡片；准备当前单元后进入既有发布与预演页，不增加配置输入。切换资料会先结束旧预演会话，不能把新节目装入旧队列会话；导航导致现有预演停止时保留检查点，后续现场会话/备播导航解耦仍需实现。
