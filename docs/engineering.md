# nai 工程与 OpenSpec 开发流程

Windows构建与缓存（2026-10-26）：先 dot-source scripts/use-project-build-cache.ps1；本轮 -Target live-auto-build，运行组合在 target-windows/live-auto-runtime，工具缓存与临时文件在 target-windows/tool-cache。其他清理任务不得删除这三个独立目录；不要使用硬编码 CARGO_HOME 到 C 盘的旧诊断脚本。详见[迁移报告](reports/2026-10-26-windows-build-cache-migration.md)。

独立桌面壳的 `TAURI_CONFIG.build.frontendDist` 必须使用相对壳清单目录的路径，例如 `../../target-windows/live-auto-runtime/frontend`。`E:/...` 可被配置解析成 URL，导致 WebView 导航至 file 地址而不是内嵌资源；编译成功不能替代原生启动冒烟。激活前执行 `scripts/test-workbench-desktop.ps1`，同时核验 PE GUI subsystem 与认证就绪握手。


当前UI工程入口（2026-10-24）：唯一前端包为apps/workbench-desktop，源码ui/，页面index.html。使用 pnpm --filter @nai/workbench-desktop dev、pnpm check:workbench-frontend、pnpm test:ui；旧apps/desktop、Engineering打包脚本及旧浏览器fixture已退役，下文旧命令仅历史追溯。新UI无旧目录依赖，check:frontend仍按包脚本运行类型/构建/测试。

新备播工作台前端使用 `pnpm check:workbench-frontend`：先编译contracts-ts，再检查新包ui/并按其Vite配置构建index.html到独立壳dist。旧UI包和旧HTML入口已移除。离线React渲染测试须让组件与renderer使用同一React实例，对齐生产dedupe。


## V4.3当前检查入口

RF01已切换唯一ACC注册表及活跃CI：`cargo xtask check --profile workbench`运行RF01–04模块/直接消费者，`cargo xtask test --suite contracts`执行新Schema无漂移、wire负例与三语言夹具；Windows另跑新原生宿主和独立桌面构建/冒烟。所有ACC仍pending。旧portable/windows-local/full命令返回LEGACY_PROFILE_RETIRED；下文历史W判据不再是新产品发布组合，不能将其成功当V4.3通过。

先构建契约包再构建新入口：`pnpm check:workbench-frontend`。Windows原生：`cargo build -p nai-workbench-engine --locked`，再`cargo build --manifest-path apps/workbench-desktop/Cargo.toml --locked`；debug用绝对路径环境NAI_WORKBENCH_ENGINE指向新engine，release要求固定同目录新engine，不回退旧程序。新GUI支持手工备播与私有ContentOnly预演，仍不是安装发布包。debug另设置NAI_FOUNDATION_ENGINE指向nai-foundation-engine绝对路径；可信宿主固定使用独立foundation-v2数据根。新冒烟脚本还需-Foundation参数，并使用临时数据根，见[工作台报告](reports/2026-10-22-v43-rf03-preparation-workbench.md)。

RF02实际接口/故障与边界见[基础说明](workbench-foundation.md)。Windows另跑`cargo test -p nai-foundation-sqlite -p nai-runtime-core --lib --locked`及`cargo test -p nai-workbench-engine --test foundation --locked`；不打开旧库或调用付费API。

## 环境与入口（旧线历史）

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
**平台连接器**（路径 `plugins/platform-*`）**不得依赖 `kernel` 层**——A03「新增连接器不改内核」的另一半：
接缝 `nai-plugin-sdk::connector` 充分则无需内核类型，依赖内核即意味着接缝有缺（W53 新增）。
反方向「内核不得依赖连接器」由普通分层规则强制（kernel 只能依赖 contracts）。

`check --profile` 的测试步不会让 harness 测到自己身上：`cargo test --workspace` 会重新链接
**正在执行**的 `nai-xtask` 映像，Windows 拒绝覆盖运行中的可执行文件（`LNK1104`）。因此检查逻辑位于
`xtask/src/lib.rs`，`main.rs` 只是 CLI 外壳，测试步为 `cargo test --workspace --exclude <包名>`
加 `cargo test -p <包名> --lib`；包名取自包自身而非字面量，改名不会留下静默的空排除。bin 目标仍被
clippy `--all-targets` 类型检查，回避的只是链接。

同一段还有一个**反直觉的坑**：手工预构建 MUST 用下面两条 `--no-run`，MUST NOT 用
`cargo build --workspace --all-targets`。后者展开为 `--lib --bins --tests --benches --examples`，
其中 `--tests` 只**类型检查**测试源码、**不产 test harness**，而 `cargo test` 恰恰要编译并**链接**
测试二进制——W146 门控里那几次 `LNK1104` 的目标正是这批 test harness 二进制。本机实测两套单元集**互不包含**：
`build --all-targets` 之后的 `cargo test --no-run` 仍会编一批包，随后 `build --all-targets` 又把同一批
判脏 ⇒ 拿它当预构建等于没有预构建。与门控**同一批单元**的预构建是：

<!-- gate-prebuild:begin -->
```bash
cargo test --workspace --exclude nai-xtask --locked --no-run
cargo test --locked --package nai-xtask --lib --no-run
```
<!-- gate-prebuild:end -->

`clippy` **不会**作废 rustc 产物（这一点也反直觉，故记录下来）：门控的 `clippy --workspace
--all-targets` 紧跟在全量 `build` 之后实测零个包重编；改动一个文件后 `build` 1 → `clippy` 1 →
`build` 0，同样不判脏。上面两个事实合起来才是「预构建为什么会失效」的完整答案，只答 clippy
那一半会把处置引到错误的落点。两条 `--no-run` 与门控两个测试步的对应关系由
`tests/contracts/gate-coverage.test.mjs` 对账（文档漏 `--no-run`、改任一旗标、门控侧换覆盖形态都会红），
实测数字见 [W147 报告](reports/2026-10-18-w147-prebuild-recipe.md)。**块外不得再出现处方**：
活文档（仓库根与 `docs/` 下，`docs/adr/`、`docs/reports/` 两段历史除外）的**围栏内**任何带
`--no-run` 的 `cargo` 命令都 MUST 是这个块里的某一条，副本一律具名失败并带路径与行号
（[ADR 0122](adr/0122-prebuild-complement-closed-world.md)）。散文里的命令不在判据域内——
围栏里的是可复制执行的，散文里的是用来说明的（本段上方那条 `build --all-targets` 反例即属后者）。

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
pnpm --filter @nai/workbench-desktop dev
```

仅在本机 127.0.0.1:5173 启动控制工程页，默认显示“运行时未连接”。可通过显式启动的本机控制示例连接真实 Session 并静音/接管（操作步骤见控制指南）；没有 Tauri 原生宿主、实际媒体设备或生产开播入口。

## OpenSpec SDD

项目上下文位于 `openspec/config.yaml`；后续重构按V4.3与[ADR0147](adr/0147-v43-product-reconstruction-baseline.md)、[V43-RF任务](v43-reconstruction-plan.md)实施，旧D/W编号只供追溯。
V4.3是后续产品/架构目标，当前安装与注册表仍是旧实现；RF01同卡归档旧registry并切换ACC/Schema/检查器，旧通过状态不自动复制。`openspec/specs/`保留已归档增量的历史，旧产品规格与V4.3冲突时以新目标/ADR为准；实现卡须显式替换相应行为/契约并记录差异，不同时承诺互斥行为。

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

`--archived` 只回答「归档过的东西能不能解析」，**不回答**「它有没有把话说完」：实测 145 份归档
proposal 里有 2 份缺段，`--archived` 对它们都是 ✓（145 / 0）。补这半边的是一条门控判据
（`tests/contracts/archived-proposal.test.mjs`）：每份归档 proposal MUST 有 `## Why` 与
`## What Changes` 且正文非空，标题只按大小写归一（`## What` / `## 为什么` 不被接受）；
不合规的 MUST 具名失败，唯一放行方式是**具名台账**，且台账 MUST 相称（声明缺口 == 实测缺口）
并**自清理**（文档补齐后条目仍在 ⇒ 失败）。理由见 [ADR 0123](adr/0123-archived-proposal-structure.md)。

`--archived` 与 `--all --strict` 各自正确，也**共同沉默**于两者**之间**：`archive` 把 delta 挪进归档
目录，但主规格那一侧没人问。实测把一条 `### Requirement:` 改名（等价于「归档了但没合入」）之后
两层仍 43 / 0 与 146 / 0。补这一格的是一条门控判据（`tests/contracts/spec-merge.test.mjs`）：
按归档目录名顺序重放 `ADDED` / `MODIFIED` / `REMOVED`，与主规格逐能力比对，**两个方向各自具名**
（归档了没合入 / 没有归档出处）；delta 的 `##` 段标记是闭世界，放不进段里的 Requirement 具名失败
而不是静默跳过。判据只判 Requirement **名**，不判正文。理由见
[ADR 0124](adr/0124-spec-merge-reconciliation.md)。

可用 `OPENSPEC_TELEMETRY=0` 禁用 CLI 遥测；CI 已设置该变量。
现有 OpenCode 指令为 `/opsx-propose`、`/opsx-apply`、`/opsx-archive`、`/opsx-explore`。
当前会话通过 CLI 完成同一流程，新增的命令/技能在重新启动 OpenCode 后加载。

## 完成证据

- 勾选任务表示该任务已实现并完成其约定检查，不表示产品所有认证通过。
- Linux/WSL、Windows 编译、设备/媒体、官方平台、真实 API、模型 Gate 分别记录。
- 尚未实现为“未开始”；外部条件阻止真实测试为 Blocked；未启用能力才可 NotApplicable。
- 不使用 mock、空成功任务或工程预览页面为真实认证标 Passed。

### 生产前端浏览器集成（显式执行）

旧 `test:frontend-bundle` 和候选fixture浏览器脚本已退役。当前新产物通过独立index.html与资源闭包检查；本轮使用Edge全新测试profile实际验证React挂载，见旧UI退役报告，不据此授予业务Ready或人工UI验收通过。




公共执行开发构建：runtime-core/storage/jsonschema/serde_json与既有media/sha2/fontdue做定向优化，保留debug断言、溢出检查及250ms观察/500ms租约。首次对应特征组合编译可能数分钟；不得通过改长期限代替复核成本治理。来源独占借用与真实实录见ADR0174。
