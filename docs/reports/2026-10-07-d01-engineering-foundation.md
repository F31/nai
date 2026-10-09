# D01 工程基础与 OpenSpec SDD

## 任务与实现状态

- 任务：D01；OpenSpec 变更：`d01-engineering-foundation`。
- 基线：V3.5 第 19、31–33 章；ADR 0001/0002。
- 实现：工程基础完成；Linux/WSL portable 实测通过。
- Windows CI：已配置，尚未执行；真实设备/SDK/GPU/安装认证未执行。
- D02–D19：仍未开始；本轮无领域消息、运行时、真实直播或平台接入实现。
- 无提交或推送；origin/main 保持远端原提交 `fb0546dc66e3c3df25181e84bdd687582bd04715`。

## 修改范围与行为

- 根 Cargo/pnpm workspace、工具链、锁文件、忽略规则及统一换行。
- `crates/nai-contracts`、`nai-kernel`、`nai-plugin-sdk`：职责文档入口，无手写领域类型。
- `xtask`：实际 metadata 检查、fmt/clippy/tests 编排、profile/未知任务拒绝。
- `apps/desktop`：React/TS/Vite 工程页，明确“运行时未连接”。
- `.github/workflows/foundation.yml`：Linux/Windows 基础工程与前端/OpenSpec 校验。
- OpenSpec config、完整 proposal/specs/design/tasks；已通过标准归档合并主规格。
- AGENTS/README、ADR、工程指南和开发计划同步。

## 环境与锁定版本

Linux/WSL 开发目录 `/mnt/e/projects/nai`。
Rust 1.96.0、Node 22.23.1、pnpm 8.15.0、OpenSpec 1.14.1、cargo_metadata 0.23.1。
前端 React/React DOM 19.2.0、TypeScript 5.9.3、Vite 7.1.9；依赖解析见真实锁文件。
未设置或调用 LLM/TTS API，本轮不需要 GPU 或官方平台 SDK。

## 实际运行命令与结果

| 命令/操作 | 实际结果 |
|---|---|
| `git ls-remote https://github.com/F31/nai.git` | main/HEAD 指向上述原提交 |
| 临时只读核对 `git ls-tree -r --name-only HEAD` | 远端仅 LICENSE |
| 本地 init/fetch/关联 origin/main 并恢复 LICENSE | 完成；保留用户文档及 OpenSpec 文件 |
| `rustup toolchain install 1.96.0 --profile minimal --component clippy,rustfmt` | 成功 |
| `cargo generate-lockfile` | 成功生成 Cargo.lock |
| `pnpm install` | 成功生成 pnpm-lock.yaml |
| `pnpm install --frozen-lockfile` | 成功，无重新解析 |
| `cargo xtask check --profile portable` | 4 个包依赖检查、fmt、clippy、workspace tests 通过 |
| `pnpm typecheck` | 桌面工程 TS 检查通过 |
| `pnpm build` | Vite production 构建通过 |
| `OPENSPEC_TELEMETRY=0 pnpm spec:validate` | D01 变更 strict 校验通过 |
| `pnpm exec openspec archive d01-engineering-foundation --yes`（遥测关闭） | 10/10 任务完成；合并 2 个规格、6 条需求并成功归档 |
| 归档后 `pnpm spec:validate`（遥测关闭） | 2 个主规格 strict 校验通过 |
| `pnpm exec openspec validate --archived --no-interactive`（遥测关闭） | D01 归档任务检查通过 |

归档：[proposal/specs/design/tasks](../../openspec/changes/archive/2026-10-07-d01-engineering-foundation/)。
主规格：`openspec/specs/engineering-foundation/spec.md` 与
`openspec/specs/spec-driven-delivery/spec.md`。

### 测试覆盖

Rust 共 **9 项测试通过**（7 项依赖检查测试 + 2 项 CLI 进程测试）：

- 合法 kernel/SDK/contracts 与 runtime 装配图。
- contracts→业务、kernel→插件拒绝。
- 插件经共享库间接依赖插件拒绝。
- metadata 层级伪装与未知路径拒绝。
- kernel 原生 SDK/Tauri 与媒体 Provider/HTTP 依赖拒绝。
- 创建真实临时 Cargo workspace，验证依赖别名以及 normal/build/dev/非活动 target 声明均被检查。
- 排除在 workspace 外的本地实现依赖拒绝。
- 启动真实 xtask 二进制，schema/certify/package/suite/未知 profile 返回退出码 1。
- Linux 请求 windows-local 返回退出码 1，不执行 Windows 检查或伪报通过。

库入口没有业务测试；9 项测试验证工程边界，不属于 T/E/U 的运行时验收。

### 发现与修复

- 初次 npx 经环境镜像下载出现 ECONNRESET；改用官方 npm registry 后成功，项目 `.npmrc` 固定官方源。
- 第一次 clippy 检查发现比较时不必要的 owned 字符串；修复后完整 portable 检查通过。

## 未测试环境与外部条件

Windows 基础编译尚未运行，不记 Passed；后续 CI 可补工程结果。
Windows 音频/窗口/安装、Tauri 原生宿主、GPU/ORT、平台 SDK、真实 API 与模型 Gate
尚未实现或未测试。相关产品任务仍未开始；真实认证缺环境时单列 Blocked。
protoc/生成器未选定，D02 负责可复现工具与 Rust/TS/C++ 生成验证。

## 契约、迁移与成本

没有领域契约或数据迁移；契约库尚无生成输出。
新增开发依赖，无新增产品常驻服务、云资源、模型下载或付费 API 成本。
前端构建产物、node_modules 和 target 已忽略，不作为仓库交付文件。

## 后续依赖

D02 新建变更，定义完整能力目录、消息信封/错误、配置和跨语言生成/兼容检查。
领域消费者在相应 Schema 建立并生成后再实现。
