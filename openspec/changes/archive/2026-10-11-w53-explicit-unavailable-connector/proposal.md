## Why

设计把「尚未取得官方接口的平台连接器」写成了**三条硬要求**：

- §9.4（第 356 行）：「快手、视频号放入**同一平台插件目录**。在核实官方接口前保留**显式 Unavailable 实现**，不宣称可上线。」
- §17.2（第 685 行）：「尚未取得官方接口的连接器**只保存契约、准入说明与显式 Unavailable 实现，不放伪造 SDK**。」
- §1344（第 1344 行）：「外部依赖缺失时：实现显式 Unavailable，报告 Blocked，继续独立的工作项；不得缩小产品能力目标，也不得用 mock 通过真实验收。」

对照仓库现状，这条要求**没有实物落点**：

- `plugins/` 下**只有 `avatar-2d`**，**没有任何平台连接器**；`apps/` 下**也没有** `platform-bridge`。
  ⇒ 「同一平台插件目录」目前是空的。
- A03「新增连接器不改内核」（`release_line: r1`，`automation: integration`）`status: implemented`，
  evidence 只有 `reports/2026-10-08-d13-platform-ingress.md`，而那份报告交付的是**平台事件准入**
  （`nai-kernel::platform::PlatformLink`，判「一个事件能不能进」）。
  **「新增连接器不改内核」这个命题目前没有主语**——没有连接器，就没有「新增连接器」。
- 「不放伪造 SDK」的**判定规则已存在**（`nai-kernel/src/bridge.rs` 的
  `fake_connector_claimed_as_real_capability`，P01 的 notes 也引用了它），但同样**没有被判定的对象**。
- 依赖分层**早就为连接器留了位**：`xtask/src/deps.rs` 的单测夹具就叫 `plugins/platform-manual`，
  断言 `nai-kernel (kernel) must not depend on platform-manual`。
  ⇒ 结构是照着「有一个平台连接器」设计的，**那个连接器一直没被造出来**。

⇒ **缺口不是「缺判定」，而是「缺被判定的实物」**。本轮把实物补上，并让 A03 的两条依赖方向都由门控强制。

## What Changes

- **连接器接缝落进 `crates/nai-plugin-sdk/src/connector.rs`**（层 `sdk`，只依赖 contracts）：
  `ConnectorIdentity`（连接器 id + 平台 id）、`UnavailableReason`（**封闭**五值）、
  `ConnectorAvailability { Verified { interface_revision } | Unavailable { reason } }`、
  `AdmissionNotes`（§13.1 合规核对项 + §9.3 接入节奏事项）、
  `trait PlatformConnector`（身份、可用性、准入说明 + §27.1 的四项桥接职责）。
  接缝自带纪律：`Unavailable` 时任何操作**只能**以 `CAPABILITY_NOT_READY` +
  `details["reason"] = "UNVERIFIED_EXTERNAL_DEPENDENCY"` 结束（§27.1 的原文术语），
  **没有**「返回 Ok 但空结果」这种表示。
- **新建 `plugins/platform-unavailable`**（层 `plugin`，依赖仅 `nai-contracts` + `nai-plugin-sdk`）：
  **快手**与**视频号**两个 `Unavailable { NoOfficialInterface }` 实例（§9.4 明文的两个平台），
  按 §13.1/§9.3 携带 in-tree 准入说明，**不含任何 SDK**。
- **「不放伪造 SDK」变成机器可查**：连接器的一条结构测试**读自身 `Cargo.toml`**，
  断言依赖落在允许集合内、且不出现 SDK 形态的依赖名。承诺 → 结构事实。
- **A03 的两条方向都由门控强制**（`xtask/src/deps.rs`）：既有规则已禁「内核依赖连接器」，
  新增「**平台连接器 MUST NOT 依赖内核**」——接缝充分则无需内核类型，依赖内核即说明接缝有缺。
- **不做**：不改 `nai-kernel` 一行（改了即自否 A03 的命题）；不写任何真实 SDK；不发网络请求；
  不建 `apps/platform-bridge`（C++ 原生桥是抖音**正式接入**的事，属 R2/P01）；不升级任何验收 `status`。

## Capabilities

### Added Capabilities

- `platform-connector`：连接器的显式不可用语义、按名失败的桥接操作、身份与准入说明、
  「不放伪造 SDK」的结构校验，以及 A03 的两条依赖方向。

## Impact

- 新增：`crates/nai-plugin-sdk/src/connector.rs`、`plugins/platform-unavailable/{Cargo.toml,src/lib.rs,tests/unavailable.rs}`、
  `docs/reports/2026-10-11-w53-explicit-unavailable-connector.md`。
- 修改：`Cargo.toml`（workspace 成员 +1）、`crates/nai-plugin-sdk/src/lib.rs`（导出模块）、
  `xtask/src/deps.rs`（新增平台连接器 → 内核的禁止项 + 单测）、`docs/platform.md`、
  `docs/platform-lifecycle.md`、`tests/registry.yaml`（A03 notes 指向落点）、`AGENTS.md`、
  `docs/development-plan.md`。
- **依赖与包数**：workspace 成员 **18 → 19**；`check-deps` 由 18 包变 19 包；
  **新增的外部依赖为零**（连接器只用 workspace 内既有的 `nai-contracts` 与 `nai-plugin-sdk`）。
- **未做边界（如实登记）**：A03 保持 `implemented`（本轮把它的命题变成**可判定**，不是把它变成
  「真实平台已接入」）；P01 仍 `blocked`；快手/视频号的真实官方接口、调试版伴侣 PoC 与上线授权
  仍是外部真值，本轮一律如实记 `Unavailable { NoOfficialInterface }`。
- **验证**：`cargo xtask check --profile full`（工程门控 + 前端）、`cargo xtask check-deps`、
  `cargo test -p nai-platform-unavailable`、`openspec validate --all --strict --archived`；
  推送后匿名复核 CI 的两平台结论。
