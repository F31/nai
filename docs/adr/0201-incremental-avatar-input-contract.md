# ADR 0201：独立连续形象输入契约

日期：2026-11-08。状态：Accepted，仅契约和消费状态机已验证；生产入口未激活。

worker6的Render完整源路径无法表达未知总长的音频流。新增nai.v561.avatarstream.v1独立输入契约，既有worker6不隐式声明支持，不保留旧格式兼容层。正式新worker装配时须独立能力握手与失败关闭，完成之前不切换实际播放路径。

控制仅含Begin、原始块引用与样本/hash、EOS及Cancel；PCM不得进入protobuf控制负载。签名覆盖确定性protobuf去mac后的全部字段。最多一个在途块，只有本地实际消费确认可释放；取消不等消费锁或GPU。EOS必须绑定实际累计样本/hash和Root已校验供应商完成，但输入EOS不等于渲染或播放完成。模型采用单任务连续滤波、音频history和motion，不逐块reset render。

Python消费状态机与Rust/Python真实编解码签名往返、连续重采样内部接线已验证。原始IPC、OS对端/ACL、worker任务循环和GPU帧时轴仍未装配；没有真实IPC或实时音画通过证据。34ACC/M0不变。
