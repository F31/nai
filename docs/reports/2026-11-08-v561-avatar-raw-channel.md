# 2026-11-08 独立原始PCM通道

## 实现

Rust v561_avatar_raw::Sender只在127.0.0.1随机端口临时监听，256bit随机私有spawn key与32byte随机nonce，通过客户端/服务端不同domain的HMAC互证；握手仅接受一条连接，不开放网络接口。key为Zeroizing，调用者只应交给明确spawn的子进程环境，不写日志/配置。握手/写入各由父Operation派生<=4秒子期限，10ms非阻塞轮询父取消，错误关闭连接并取消父任务。没有OS PID/ACL身份认证，不把应用密钥互证称为Windows进程身份。

原始帧为u32le protobuf Chunk头长（1..256）、Chunk编码、samples*2个裸PCM字节。PCM不进控制protobuf/JSON。每块<=38400样本，Sender单pending，只有调用者认证签名CHUNK_CONSUMED后才可释放。Python avatar_raw_channel独立读取，10ms检查取消，复检重复头和已签名Chunk完全一致，失败关闭socket。

消费状态机改为stage签名元数据并占槽→独立receive实际PCM核对hash→本地consumed回执，避免控制线程等待PCM。原始读不持控制锁。修复上一切片的EOS生命周期问题：EOS关闭输入但不关闭执行取消；EOS后仍可认证Cancel。该修复有状态测试，未将仅CPU的fixture声称为生产GPU执行循环。

## 验证

- Python8项状态/数值测试通过（包括新增staged块取消、未收PCM不能消费确认、EOS后取消）。
- Python4项真实本机socket故障测试通过：头长/身份、半包取消并关闭、EOF/外部地址拒绝、错误服务端证明拒绝。
- Rust3项真实Windows子进程测试通过：随机密钥互证、签名stdin/stdout控制与独立TCP原始PCM、117120样本分4块→同一Python重采样器→逐块签名消费→EOS样本/hash一致；只送1字节PCM的半包场景仍可通过控制Cancel，断言回执<500ms且退出0；错误客户端key被拒、Root父任务取消并关闭端点。
- Rust1项签名protobuf→Python→Rust既有互通回归通过。共16项，不含GPU、设备播放或OBS。
- 对应库clippy -D warnings通过；只存在既有E盘增量缓存硬链接复制回退工具链警告。协议源和生成产物未改，不重复无关全量生成/工作区测试。
- 首次沙箱socket测试被WinError10013拒绝，原生范围重跑通过；修复测试listener失败清理，未改系统代理/防火墙。独立Rust测试产物为target-windows/v561-clock-build/raw-channel-tests*.exe，测试fixture为tests/v561/avatar_stream_pipe_child.py（明确测试专用，不是生产worker）。

## 边界和下一任务

Root发送与Python读取组件已经真实独立进程验证，生产GPU worker6仍未装配该通道，worker启动能力握手/新任务循环、OS PID/ACL和完整新入口发布组合未验。当前生产默认不能声明增量能力Ready。没有调用LLM/TTS/GPU、没有播放、录制、推流或更新安装版，没有修改数据库/Key。

下一项将同一重采样器、音频history和motion接入真正GPU任务循环，保留控制独立、首帧回执、raw单槽和帧共享内存所有权；明确未知源总长、EOS尾帧截断与失败取消。再做真实CosyVoice块→GPU帧→设备时钟音画；不通过逐块reset render模拟连续。34ACC pending、M0 No-Go不变。
