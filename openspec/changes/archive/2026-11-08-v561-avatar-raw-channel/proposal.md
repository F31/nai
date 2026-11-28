# Proposal

## Why
增量契约与连续滤波已完成，但此前跨语言测试用文件传protobuf并在fixture内提供PCM，不能证明实际控制/媒体分离，也不能证明半包卡读时取消仍可处理。需要实现真实独立原始TCP通道并验证该边界，再装配GPU。

## What Changes
新增仅本机的原始通道组件：challenge/HMAC绑定私有spawn密钥、protobuf元数据头加独立PCM、有限长度及可取消分段读。消费状态机允许先校验签名控制并占单槽，再接收PCM核对摘要，消费后释放。EOS只关闭输入，不禁止仍在执行的任务取消。

## Impact
内部消费API和独立传输组件/原生测试，不修改已生成协议、worker6、安装版或GPU能力声明。测试不代表生产worker已装配。
