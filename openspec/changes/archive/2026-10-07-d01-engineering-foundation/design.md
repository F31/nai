# Design

## Context

见 proposal.md。远端 main 仅 MIT LICENSE；本地已有 V3.5、准备文档与 OpenSpec 1.3.1 生成的 OpenCode 集成文件，保留用户文件。当前为 Linux/WSL，无已建领域消费者。

## Goals / Non-Goals

**Goals:** 可锁定安装、编译和检查的 D01 工程，以及先规格后实现的 SDD 闭环。

**Non-Goals:** D02 的领域消息和生成、D04 内核业务、D14 Tauri 原生宿主及直播 UI、D19 安装认证。

## Decisions

1. Rust 固定 1.96.0（当前编译器版本），Node 固定 22.23.1，pnpm 固定 8.15.0；
   OpenSpec 固定 1.14.1 为根开发依赖。相对浮动 stable/latest，可在 CI 重现本轮组合。
   保留现有 OpenCode 集成；通过 `pnpm exec openspec` 执行项目锁定 CLI。
2. 最小库入口为 nai-contracts、nai-kernel、nai-plugin-sdk，只提供职责文档，不手写领域类型或提前消费契约。
   相对一次创建所有空目录/服务，后续任务按真实实现加入 workspace。
3. `nai-xtask` 使用 cargo_metadata 读取 `--locked --no-deps` 元数据；检查每条直接依赖（包括 build/dev/target）和 workspace 可达路径。
   每个包用 `[package.metadata.nai] layer` 声明层级，路径必须对应角色；不以别名或改 metadata 绕过。
   contracts 不依赖 workspace 业务；kernel 只允许 contracts；plugins 不能依赖插件实现，包含间接实现依赖；
   media 不依赖 Provider、Provider 插件或 HTTP 客户端；runtime 负责后续具体装配。
   D01 不假装已经实现前端 SQL/凭据权限认证，前端源码与 Tauri 权限在 D14 另验。
4. portable 检查实际执行 fmt/clippy/test/check-deps；windows-local 使用同一检查，限 Windows 原生运行，输出基础检查边界。
   不实现无工作的成功子命令。暂不提供全部 suite/profile。
5. `apps/desktop` 建立 React/Vite 工程状态页，明确未连接运行时；不展示假事件/媒体/权限。
   Tauri 原生宿主由 D14 实现；当前 frontend typecheck/build 有实际输入。
6. GitHub Actions 的 Linux/Windows runner 执行相同基础检查与前端构建/OpenSpec strict validation；
   不将 Windows hosted runner 的工程编译称为 Windows 设备验收。

## Risks / Trade-offs

- 固定当前工具链可能遇到下载/镜像问题 → 使用官方 registry，真实记录故障与最终结果。
- 初始图较小不能证明所有未来边界 → 用合成违规图测试关键拒绝分支，每轮新增包继续执行 metadata 检查。
- 空库入口可编译但无业务 → 包文档、工程页和计划保留未实现状态，不宣称 D02/D04 完成。
- 现有 OpenCode 指令版本与 CLI 不同 → 保留生成文件，本轮实际流程以锁定 CLI 输出为准；后续必要时显式刷新。

## Migration Plan

本轮仅新增工程与文档，无运行数据迁移。关联既有 origin/main 并保留 LICENSE，无提交或推送。
检查通过后使用 OpenSpec 标准归档同步主规格，D02 新建独立变更。

## Verification

- `cargo xtask check --profile portable`（fmt、clippy、workspace tests、实际 metadata 检查）
- `pnpm install --frozen-lockfile`、`pnpm typecheck`、`pnpm build`
- `pnpm exec openspec validate --all --strict --no-interactive`
- 非 Windows 下 windows-local 和未知任务的退出码检查
- Windows 基础编译交给 CI；媒体、安装、设备、SDK、GPU 未实测单列。
