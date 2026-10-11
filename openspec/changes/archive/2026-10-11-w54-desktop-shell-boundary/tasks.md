# 任务（w54 桌面壳形态与 Tauri 边界）

## 0 立项

- [x] 0.1 建立 `openspec/changes/w54-desktop-shell-boundary/`（`.openspec.yaml` / `proposal.md` /
      `design.md` / `specs/desktop-shell/spec.md` / `tasks.md`）
- [x] 0.2 `openspec validate w54-desktop-shell-boundary --strict` 通过

## 1 ADR 0046

- [x] 1.1 `docs/adr/0046-desktop-shell-form-and-tauri-boundary.md`：形态（Tauri 2，§1248）、
      壳只做四件事与「什么不算壳」、IPC 双通道（§1101）、capabilities/CSP 最小权限（§1151）、
      生命周期与 sidecar 三元组（§1259）、数据根（§900）、打包与签名、三形态共用边界、
      已知后果（R1–R6）与不升级
- [x] 1.2 明确引用已就位的 INV-18（`xtask/src/deps.rs` 的 `tauri` 前缀检查），**不重复实现**

## 2 声明式落点（Tauri 2 真实格式）

- [x] 2.1 `apps/desktop/src-tauri/tauri.conf.json`：`frontendDist` 指向 `../dist`、
      `app.windows[0].label = "main"`、`app.security.csp` 逐指令显式（含 `ipc:` 与 `http://ipc.localhost`，
      无 `unsafe-eval`、无通配源）、`bundle.externalBin` 以**不带三元组的逻辑名**声明 sidecar
- [x] 2.2 `apps/desktop/src-tauri/capabilities/default.json`：`permissions` **逐条**列举 5 条
      `core:<模块>:allow-<动作>`，**不使用**任何聚合权限集（如 `core:default`）、无 shell/fs/sql/process
- [x] 2.3 不加 `tauri`/`@tauri-apps/*` 依赖、不建 Rust 主体、不改 workspace members（保持 19 包）
- [x] 2.4 另加 `shell-policy.json`（本仓库自有的策略声明，非 Tauri 字段）与 `README.md`
      （说明「只有声明、没有 `Cargo.toml`/`main.rs`」是裁决结果）

## 3 校验（`tests/ui/desktop-shell.test.mjs`，零新增依赖）

- [x] 3.1 权限集合与规格声明的 5 条**逐一相等**（多一条、少一条都失败）；标识符形状必须匹配
      `^core:[a-z]+:allow-[a-z0-9-]+$`（`core:default` 一类聚合权限被拒）
- [x] 3.2 全部权限标识符不得命中 shell / fs / sql / process 语义
- [x] 3.3 CSP：无 `unsafe-eval`；任一指令不得使用 `*` 或通配协议源（`http:`/`https:`/`ws:`/`wss:`）；
      `connect-src` 必须含受保护 IPC 的源
- [x] 3.4 sidecar：`bundle.externalBin` 的逻辑名**不带**目标三元组且须对应 `apps/` 下**真实存在**的二进制包；
      命名规则 `包名-目标三元组(+.exe)` 对四个目标三元组逐一成立
- [x] 3.5 数据根：`src-tauri/` 配置与 `apps/desktop/src/**` 源码中不得出现硬编码绝对路径
      （回环 URL **不**误报）
- [x] 3.6 前端源码不得 import `@tauri-apps/api` 与 `@tauri-apps/plugin-{fs,shell,sql,process}`
- [x] 3.7 壳的策略声明：四件事范围、业务逻辑 forbidden、后台生命周期不绑界面、IPC 角色、三形态集合
- [x] 3.8 **非空洞守卫**：注入 `shell:allow-execute`、`core:default`、`unsafe-eval`、`*`、`http:`、
      未登记 sidecar、`@tauri-apps/plugin-fs`、`backendsOutliveUi=false`、`C:\...`、`/home/...` 等样本，
      各检查**分别抓到**
- [x] 3.9 该文件被既有 `test:ui`（根 `package.json` 的 `scripts`）覆盖，自动进入 `full` 门控
      —— `tests/ui` 由 **37 → 52**（+15），**未改门控代码**

## 4 registry 与文档

- [x] 4.1 `tests/registry.yaml` 新增 **A12「桌面壳宿主边界」**：`release_line: r1`、`automation: manual`、
      `status: pending`，notes 写明**本轮落点**与**缺失前置**（真实依赖、可运行壳体、打包与签名链），
      并记 R1/R2/R3 三条已知后果
- [x] 4.2 `docs/control.md` 就地订正（受保护 IPC 为正式路径、HTTP 为测试适配）＋ 新增边界表
- [x] 4.3 `docs/ui.md` 增「桌面壳宿主边界（W54）」小节
- [x] 4.4 `apps/desktop/src/main.tsx` 页脚改准确（形态与边界已裁决／**壳体尚未构建**；基线 V3.6 → V3.7）
- [x] 4.5 `AGENTS.md`、`docs/development-plan.md` 同步；`.gitignore` 预置 `src-tauri/{target,gen}/`
- [x] 4.6 `docs/reports/2026-10-11-w54-desktop-shell-boundary.md`（含 19 包不变、零新增依赖、
      Rust 侧零改动、R1–R6 已知后果与未做边界）

## 5 闭环

- [x] 5.1 预热两个测试目标后跑 `cargo xtask check --profile full`：**EXIT=0**、
      **86 目标（68 `Running` + 18 `Doc-tests`）、740 passed / 0 failed**、前端 6 步全过
      （本轮零 Rust 改动，数字与 W53 持平；前端增量在 `tests/ui`）
- [x] 5.2 `cargo xtask check-deps` 仍为 **19 workspace packages**
- [x] 5.3 全部 tasks 勾选后 `openspec archive -y w54-desktop-shell-boundary`
- [x] 5.4 `openspec validate --all --strict --archived` 记录条数
- [x] 5.5 一次 commit + push（提交信息用 Write 落 UTF-8 文件，避免 `\u` 字面转义）
- [x] 5.6 用**缓存破坏参数**匿名复核 CI：断言 `head_sha` 与推送一致、记录 conclusion 与失败步骤
