## Why

`tests/registry.yaml` 的 **S11「IPC 模糊与路径穿越」** 状态是 `tested`，但同一行的 notes 写着「帧/握手/路径测试通过；**模糊测试待建**」。
全仓库 `grep -ri fuzz` **零命中**——验收项名字里的「模糊」这一半**没有实现**，却被标成 `tested`。这是**登记与事实不一致**，
比缺功能更危险：读者会以为攻击者可控输入的健壮性已被验证。

IPC 面上实际有三处**攻击者可控的字节入口**（都在解码路径上）：

| 入口 | 代码 | 输入来源 | 现有测试 |
| --- | --- | --- | --- |
| 帧长度前缀 | `crates/nai-plugin-sdk/src/frame.rs:41` `read` | 对端任意字节 | 6 项定向单测 |
| `Envelope` / `CapabilityInvocation` / `CancelRequest` 解码 | `crates/nai-plugin-hosts/src/host.rs:154,181,195` | 帧 body | 无 |
| `Handshake` 解码与字段校验 | `crates/nai-plugin-sdk/src/handshake.rs:33` `accept` | 首帧 body | 2 项定向单测 |

外加路径面：`crates/nai-plugin-hosts/src/boundary.rs:32` `verify_within_root`（仅 1 项定向单测，用例手写）。

定向测试证明的是**已想到的形状**被正确处理；它不证明「任意字节都不会 panic / 不会越界分配 / 错误是具名的」。
本轮补上后者。

## What Changes

- 新增 `crates/nai-plugin-hosts/tests/untrusted_input.rs`：**确定性、有界、可复现**的随机输入测试，覆盖上表三个解码入口与路径面。
- 断言口径是**全域性与有界性**（任意字节 ⇒ `Ok` 或**具名错误码**，不 panic、不按输入分配超上限），并钉住「**先取消/期限，后看内容**」的顺序不变量。
- 测试**自带非空洞性守卫**：语料必须实际命中若干具名错误码与若干危险形状（超长长度、截断、合法往返、版本不符、未知消息类型、穿越路径…），
  计数不足即**测试失败**——防止生成器只产合法输入把测试变成橡皮图章。
- `tests/registry.yaml` S11 notes 记录本轮证据与**边界**（非覆盖率导向、不含 libFuzzer）；状态保持 `tested`，但这次**有据可依**。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `plugin-hosts-and-ipc`：新增三条需求——攻击者可控输入面 SHALL 有确定性随机输入测试且断言全域性/有界性/非空洞性；
  取消与期限 MUST 先于内容解释；路径包含校验 SHALL 在随机输入下仍保证不出根目录。

## Impact

- 代码：**新增 1 个测试文件**；`frame.rs` / `handshake.rs` / `host.rs` / `boundary.rs` 实现**预期零改动**（若harness 发现真实缺陷，则修实现并记录——如实为准）。
- 依赖：**零新增**（自带 splitmix64 PRNG；不引入 `cargo-fuzz`/`libFuzzer`/`arbitrary`）。18 包与依赖方向不变。
- 证据：`docs/reports/2026-10-11-w39-ipc-untrusted-input.md`；`docs/hosts.md` 补一节。
- 验收：S11（`plugin-hosts-and-ipc`）；不改其他登记状态。

## Non-Goals

- **不做覆盖率导向模糊测试**：`cargo-fuzz`/`libFuzzer` 需要 nightly 与新增依赖，且会引入独立 target/语料目录；
  本轮明确用**确定性种子 + 结构化语料生成**，并在报告与 registry 中**如实写明这是更弱的东西**，不得称为「已做模糊测试」。
- 不测真实命名管道/Unix socket 的对端身份（`SO_PEERCRED` 在本钉住的 rustc 上仍不稳定，见既有记录）；不发起真实子进程。
- 不新增仓库级证据目录：**§30.1 未为模糊测试指定目录**，per-crate `tests/` 是既有惯例（`tests/e2e.rs` 等），不臆造 `tests/fuzz`。
