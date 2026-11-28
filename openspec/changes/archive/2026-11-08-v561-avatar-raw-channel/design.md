# Design

Root临时监听127.0.0.1随机端口，只接受私有子进程的256bit spawn key对随机32byte挑战的HMAC-SHA256响应；原始数据端点从不监听外部接口。相同密钥另为控制Request签名，不使用无保护共享文件。Raw帧为u32le protobuf Chunk头长（1..256）、Chunk编码、samples*2个裸PCM字节，重复元数据必须与已签名控制一致。控制线程先stage并推进序号，单槽防积压；原始读线程不得持控制锁阻塞。读取10ms检查取消/单调期限，PCM<=76800bytes。消费完成后本地ack释放；取消在半包和EOS后都可到达。此阶段仅组件及真实Rust/Python子进程验证，尚无生产raw握手装配、OS PID/ACL或GPU通过；应用HMAC对端绑定不等于OS对端身份。
