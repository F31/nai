# nai 工程与 OpenSpec 开发流程

## 环境与入口

固定 Rust 1.96.0（含 rustfmt/clippy）、Node 22.23.1、pnpm 8.15.0。
Rust workspace 使用 `nai-*`，前端使用 `@nai/*`；详见 ADR 0001/0002。

```bash
rustup toolchain install 1.96.0 --profile minimal --component clippy,rustfmt
pnpm install --frozen-lockfile
cargo xtask check --profile portable
pnpm typecheck
pnpm build
pnpm spec:validate
```

Windows 原生环境使用 `cargo xtask check --profile windows-local`。
目前这两个 profile 都只检查工程基础。Windows hosted CI 通过也不代表音频设备、
直播伴侣、GPU、官方 SDK 或干净机安装验收。

可单独运行 `cargo xtask check-deps`。CLI 依据真实 metadata 检查所有 normal/build/dev/target
声明、路径层级和 workspace 可达关系。库/插件新增时登记 workspace member 与
`[package.metadata.nai] layer`；新增路径职责必须增加规则与必要测试。

`check --profile` 的测试步不会让 harness 测到自己身上：`cargo test --workspace` 会重新链接
**正在执行**的 `nai-xtask` 映像，Windows 拒绝覆盖运行中的可执行文件（`LNK1104`）。因此检查逻辑位于
`xtask/src/lib.rs`，`main.rs` 只是 CLI 外壳，测试步为 `cargo test --workspace --exclude <包名>`
加 `cargo test -p <包名> --lib`；包名取自包自身而非字面量，改名不会留下静默的空排除。bin 目标仍被
clippy `--all-targets` 类型检查，回避的只是链接。
内存受限的宿主可用 cargo 原生开关约束并发（如 `CARGO_BUILD_JOBS=2 cargo xtask check --profile portable`）：
默认并发下全量链接 30 余个带 debuginfo 的测试二进制可能触到系统提交限制（本机实测 `os error 1455`），
那是宿主资源而非仓库缺陷，故未写进 portable 检查。
若宿主同时很慢的文件系统，还需约束**测试**并发（`RUST_TEST_THREADS=4`，libtest 原生读取）：
`O(memory)` 的单个操作有 30 秒硬上限（`Operation::new` 拒绝更长超时，属产品不变量，不可放宽），
而本机实测「打开一个 Store 并完成迁移/权限装配」固定约 5 秒、`backup`+`restore` 再加约 7 秒——
workspace 全局并行时单次操作会被挤压逼近该上限，于是出现 `DeadlineExceeded`。两者都是宿主吞吐事实，
以文档记录；仓库代码不做平台或资源特化。

需要 Unix 进程模型或原生子进程管道的演练用文件级 `#![cfg(target_os = "linux")]` 声明范围
（`apps/output-keeper/tests/frame_drill.rs`、`tests/faults.rs`、`apps/render-worker/tests/usage.rs`）；
在受支持平台仍缺少宿主能力时必须给出具名诊断，不得以 `ok` 报告未执行的演练。只在特定平台机制下成立的
规则（例如 Unix mode 位）用特征化断言钉住缺口，不静默跳过。平台事实与真缺陷的区分依据见
[报告](reports/2026-10-10-engineering-loop-platform-truths.md)。

contracts 已由 D02 提供生成消息和结构检查；kernel 注册/绑定与 SDK 清单转换已由 D04 实现，
宿主/IPC 与 SDK 帧/握手/ACL 已由 D05 实现，会话/epoch/取消及媒体租约门控已由 D06 实现。
有界调度/公平/债务和共享预算持久账本已由 D07 实现。
API Provider 传输、SSE/二进制流解析、有界重试/取消与 usage 接线已由 D08 实现；真实计费受账户余额约束。
真实媒体循环/设备音轨与实际业务插件仍待后续任务。
已实现 `cargo xtask schema [--check]` 与 `cargo xtask test --suite contracts`。
前置工具、生成产物和完整 suite 说明见 [契约指南](contracts/README.md)。
已实现 `cargo xtask test --suite storage`，覆盖迁移、事实/权限、快照与凭据边界，详见 [存储指南](storage.md)。
已实现 `cargo xtask test --suite kernel`，覆盖注册、闭包、会话绑定和清单SDK，详见 [内核指南](kernel.md)。
已实现 `cargo xtask test --suite hosts`，覆盖帧/握手/ACL/生命周期与加载隔离，详见 [宿主/IPC 指南](hosts.md)。
已实现 `cargo xtask test --suite session`，覆盖会话/命令幂等、两层取消、媒体租约复检和存储撤销 CAS，详见 [会话指南](session.md)。
已实现 `cargo xtask test --suite scheduler`，覆盖调度、共享预算/并发、结算/对账与持久派发 claim，详见 [调度指南](scheduler.md)。
已实现 `cargo xtask test --suite provider`，覆盖传输/重试/取消/SSE/二进制流与 usage 接线，详见 [Provider 指南](provider.md)。
已实现 `cargo xtask test --suite knowledge`，运行 knowledge-core 与 storage，覆盖中文/向量检索、发布 CAS/冻结绑定/恢复、远端契约/政策门控与 rerank，详见 [知识指南](knowledge.md)。
W47 新增 `cargo xtask sbom [--out <path>]`：从 `cargo metadata --locked` 与 `Cargo.lock` 的 SHA-256 生成
CycloneDX 1.5 物料清单（**零新增依赖、离线、逐字节确定**；许可只在能断言时记为 SPDX id/expression，
否则原样记为 name；产物自带范围与「不能用作什么」声明）。它是产出命令，**不在 `check --profile` 内**；
其单测在 `cargo test -p nai-xtask --lib` 内运行。见 [报告](reports/2026-10-11-w47-sbom-supply-chain.md) 与 [ADR 0043](adr/0043-sbom-generation-and-scope.md)。
D10 新增 `nai-kernel::evaluate` 纯逻辑智能体/守门评估（decision 路由、四阶段 guard、skill/memory/show），详见 [内核指南](kernel.md)；实际守门插件由插件层实现。
D11 新增 `nai-media-core::timeline` 可移植媒体时序内核（audio/clock/ring/resample/record/ack 与双水位 commit），`cargo test -p nai-media-core` 覆盖，详见 [媒体指南](media.md)；Windows 窗口/音轨设备仍 Blocked。
W06 前置新增 plugin 层 `nai-avatar-2d`：素材许可/hash/闭包准入、样本时轴RMS平滑嘴型和实际RGBA帧合成；`cargo test -p nai-avatar-2d --offline` 可执行。详见 [2D指南](avatar-2d.md)；独立worker/共享内存/root接线未实现，软件测试不替代Windows帧率/设备认证。
W05/W06 前置新增 `nai-media-core::frame_slots` 与 Linux 显式 `nai-shared-frame-drill`：有界单owner槽位、代次隔离、SHA256完整性、超时/断连退休整池与真实继承FD跨进程传输；`cargo test -p nai-media-core --offline` 覆盖。详见 [媒体指南](media.md) 与 [ADR 0017](adr/0017-frame-slot-drill-boundary.md)；tmpfs-file是有复制的工程后端，非mmap/正式共享内存，独立worker/keeper生产装配与Windows映射未实现。
D12 前置新增 SDK 模型声明/内容完整性检查，使用 `cargo test -p nai-plugin-sdk --offline` 验证；见 [模型指南](models.md)。真实写实推理/导出尚未实现，G0–G3 未执行。
D13 新增 `nai-kernel::platform` 官方事件准入与消费复检，`cargo test -p nai-kernel --offline` 覆盖，见 [平台指南](platform.md)。真实 SDK/生产宿主接线与网关服务尚未实现，逻辑实测不等于官方准入。
D14 首轮新增 `nai-control-api` 显式 loopback 控制服务和桌面客户端，`cargo test -p nai-control-api --offline`、`pnpm --filter @nai/desktop test` 与 `pnpm test:control-smoke` 可执行；见 [控制指南](control.md)。W09 新增真实 Store 注入的 list/approve/reject API、候选入库上限及原子决策日志；正式 Tauri/受保护 IPC、完整预检/审批 GUI 与生产 root 接线尚待实施。
其他 suite、`certify` 和 `package` 尚未实现，调用会非零退出。

## 前端工程预览

```bash
pnpm --filter @nai/desktop dev
```

仅在本机 127.0.0.1:5173 启动控制工程页，默认显示“运行时未连接”。可通过显式启动的本机控制示例连接真实 Session 并静音/接管（操作步骤见控制指南）；没有 Tauri 原生宿主、实际媒体设备或生产开播入口。

## OpenSpec SDD

项目上下文位于 `openspec/config.yaml`；每轮按 V3.7 工作项及 registry 验收选一个完整可验证变更，D 编号只供追溯。
V3.7 是唯一实现基线（ADR 0042；ADR 0015 原升 V3.6），`openspec/specs/` 是已归档增量规格；有冲突时先澄清并记录 ADR。

1. 新建变更：`pnpm exec openspec new change <name> --schema spec-driven`。
2. 读取当前阶段：`pnpm exec openspec status --change <name> --json`。
3. 读取模板/规则：`pnpm exec openspec instructions proposal --change <name> --json`，
   同样获取 specs、design、tasks 指令；先编写可验证需求和场景再编码。
4. 校验：`pnpm exec openspec validate <name> --strict --no-interactive`。
5. 实施：`pnpm exec openspec instructions apply --change <name> --json`，
   依据返回的 contextFiles 和 tasks 实施；每项完成验证后立即勾选。
6. 同步 `docs/development-plan.md` 与 `docs/reports/`，记录命令、环境、失败与修复。
7. 完成后归档：`pnpm exec openspec archive <name> --yes`，合并 delta specs 并保留变更历史。
8. 验证主规格和归档：`pnpm spec:validate`、
   `pnpm exec openspec validate --archived --no-interactive`。

可用 `OPENSPEC_TELEMETRY=0` 禁用 CLI 遥测；CI 已设置该变量。
现有 OpenCode 指令为 `/opsx-propose`、`/opsx-apply`、`/opsx-archive`、`/opsx-explore`。
当前会话通过 CLI 完成同一流程，新增的命令/技能在重新启动 OpenCode 后加载。

## 完成证据

- 勾选任务表示该任务已实现并完成其约定检查，不表示产品所有认证通过。
- Linux/WSL、Windows 编译、设备/媒体、官方平台、真实 API、模型 Gate 分别记录。
- 尚未实现为“未开始”；外部条件阻止真实测试为 Blocked；未启用能力才可 NotApplicable。
- 不使用 mock、空成功任务或工程预览页面为真实认证标 Passed。
