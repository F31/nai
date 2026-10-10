# 插件宿主、IPC 与隔离

```bash
cargo xtask test --suite hosts
cargo xtask check --profile portable
```

suite 实际测试 nai-plugin-sdk 与 nai-plugin-hosts；真实端到端握手/调用使用
TcpStream 回环（可移植）与 UnixStream 对（unix，含 OS peer 凭据）。

## 帧与传输

`nai_plugin_sdk::frame` 提供有界帧：4 字节小端长度 + body，默认 64KiB。
- 长度超过上限在分配前拒绝；部分头、部分帧体 EOF、多帧合并、逐字节读取均正确处理。
- `read` 携带 deadline 与共享取消标记；干净 EOF（帧边界）返回 None，中途 EOF 报错。
- `write` 编码完整帧。生产本地总线应由 D14/D15 提供真正的 Unix 域套接字或 Windows 命名管道。

## 握手与 ACL

`handshake::accept`（服务端）要求首帧为 Handshake，校验 plugin_id/version/nonce/host、
协议范围，经 `IdentityValidator::validate` 得到 principal，协商版本交集，回显 nonce。
`handshake::connect`（客户端）校验 ack nonce 与协商结果。握手后所有帧都是 Envelope。

`AccessPolicy` 按 session/principal/capability/operation 精确授权；`CapabilityHost`
在调用插件前检查 ACL，空策略默认拒绝。身份与 ACL 均由可信 composition root 提供，
不读取插件自报身份；OsPeer 仅在 Unix 域套接字通过 peer_cred 提取，None 不视为已认证。

## CapabilityHost 与 Plugin

- `CapabilityHost::serve`：握手 → 循环读 Envelope → 校验协议主版本 → 取消前置拒绝 →
  ACL 分派 → 写 CapabilityResult（data/complete/error 明确结束）。未知消息类型报错关闭连接。
- `Plugin` trait（object-safe、Send+Sync、同步）：describe / validate_config / start /
  health / suspend / resume / stop / invoke。未 start 不得返回健康通过；suspend 期间
  调用被拒绝；停止后重新启动须重新验证。
- `PluginContext` 只暴露 host 注入的期限、取消、预算、获准配置与 credential refs。
- `CapabilityClient` 在握手后发送 CapabilityInvocation，收集结果直到明确 StreamEnd，
  带 chunk 上限；`cancel` 记录 request_id，后续匹配调用分派前拒绝 CANCELLED。

同步 invoke 不能抢占进行中计算；真正异步取消与流式调度属于 D06 的执行工作。

## 加载与原生隔离

- `verify_within_root(root, path)`：拒绝 `..`、绝对逃逸与符号链接/重解析点逃逸，
  规范化后必须位于根内。插件/模型/原生 DLL 只能从已验签根目录加载。
- `scrub_env`：移除 token/secret/password/credential/apikey 类环境变量，防止子进程继承。
- `NativeBoundary::verify_subprocess_sandbox`：无经认证的 OS 沙箱证据时显式
  `CAPABILITY_NOT_READY`（Unavailable）。不可信原生子进程必须在 AppContainer/受限身份
  等经认证边界运行并验证文件/网络/凭据限制；无法满足则拒绝，不使用同用户进程冒充沙箱。

## 随机输入测试（S11「模糊」，W39）

`crates/nai-plugin-hosts/tests/untrusted_input.rs` 用**确定性种子**的随机语料打击**攻击者可控**的输入面：
帧长度前缀、握手 body、Envelope/CapabilityInvocation/CancelRequest 解码，以及 `verify_within_root` 的路径字符串。

它与上面的定向测试是**两件事**：定向测试证明「**已想到的**形状被正确处理」，随机测试断言**全域性与有界性**——
任意字节只产生 `Ok` 或**具名 `ErrorCode`**，不 panic、不按输入无界分配；并且撤销/期限**先于**内容解释。

三点必须说清楚：

- **这不是覆盖率导向的模糊测试。** 没有 `cargo-fuzz`/libFuzzer、不用 nightly、**零新增依赖**；
  探索能力显著弱于真 fuzzer。名字骗人就是证据骗人，故一律称「随机输入测试」。
- **阴性结果不是安全证明。** 本 harness 未发现缺陷，只说明「这批语料没打出来」，**不**说明「不存在缺陷」。
- **非空洞性守卫是它成立的前提。** 生成器若只产合法输入，全部断言都会通过而什么都没证明。
  故每类语料都统计**实际命中**的形状与错误码，计数为零即**测试失败**。已实测的命中计数（固定种子）：

  | 语料 | 命中 |
  | --- | --- |
  | 帧流（4096 次） | `bodies=683`、`clean_eof=179`、`oversized=1982`、`truncated=1935` |
  | 结构化 Envelope（2048 次） | `invocation_decoded=1569`、`version_incompatible=601`、`invalid_argument=846`、`denied_or_absent=225`、`completed=185`、`cancel_acks=191` |
  | 路径串（2048 次） | `rejected=1927`、`accepted=121`、`traversal_rejected=651` |

  守卫本身做过**反向自检**：把阈值调到不可能命中，两个测试**确实失败**并打印计数，随后恢复。

一个容易读错的语义（本轮踩到）：**ACL 拒绝与能力缺失是「写到对端的 result 级错误」，不是连接级错误**——
`CapabilityHost::dispatch` 写完 `CapabilityResult{end: StreamError, error: PermissionDenied}` 后返回 `Ok(())`，
所以 `serve` 返回 `Ok(())` 并不等于「被放行」。只看 `serve` 的返回值会把拒绝**误计为成功**。
harness 因此解析宿主**写出的帧**（并顺带断言宿主只输出良构帧），才能看到这 225 次拒绝。

```bash
cargo test -p nai-plugin-hosts --offline
# 8 项随机输入测试 + 6 项 lib 单测 + 3 项 e2e；实测约 0.5s，不构成门控瓶颈
```

## 当前边界

Linux/WSL 已实测帧、握手、ACL、端到端调用、路径守卫与环境清理。Windows 命名管道
peer PID、真实 Named Pipe、AppContainer/Job Object、受限身份与文件/网络/凭据限制
仍未实测（Blocked），需 Windows 设备验收。D05 不提供原生故障隔离或 P0 静音。
