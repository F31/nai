# `src-tauri`：桌面壳的形态声明与最小权限面

本目录**只有声明**，**没有** `Cargo.toml`、没有 `main.rs`、没有构建产物。这不是遗漏，是裁决结果
（[ADR 0046](../../../docs/adr/0046-desktop-shell-form-and-tauri-boundary.md) D3）：

- 引入真实 `tauri`/`tauri-build` 会把 **WebView2 的导入库与网络**拉进构建链，
  从而使 `cargo xtask check --profile portable|full` 的结论取决于**主机事实**（是否装 WebView2、网络是否可达）。
  这是 [ADR 0041](../../../docs/adr/0041-engineering-profile-frontend-boundary.md) 已裁过的禁忌。
- 因此本轮的产出是**真实格式的声明**与**自建的形状校验**，不是一个可运行的壳体。

| 文件 | 作用 |
| --- | --- |
| `tauri.conf.json` | Tauri 2 的**真实**配置格式：窗口、`frontendDist`、逐指令显式 CSP（无 `unsafe-eval`、无通配源）、sidecar 逻辑名 |
| `capabilities/default.json` | 最小权限面：**逐条**列举 5 个具体命令；**不用**任何聚合权限集；无 shell/文件系统/SQL/进程控制 |

**这两个文件当前不被任何构建消费。** 配置落地 **MUST NOT** 被读作壳已实现：
`tests/registry.yaml` 的 A12 保持 `pending`，缺失前置是**真实依赖、可运行壳体、打包与签名链**。

## 校验

```bash
node --experimental-strip-types --test tests/ui/desktop-shell.test.mjs   # 在仓库根执行
```

该文件被根 `package.json` 的 `test:ui` 覆盖，因此自动进入 `cargo xtask check --profile full`。
校验读的是配置的**形状**，**不能**证明它被某个 Tauri 版本接受（ADR 0046 R1）；
`$schema` 指向官方 URL，**离线不可校验**（R3），故校验自建、不依赖网络。

## 与前端的关系

`apps/desktop/src/` 是**不依赖 Tauri 的纯 Web 前端**（校验会断言它不 import `@tauri-apps/*`）。
壳体一旦引入，同机控制通道 **MUST** 迁到受保护 IPC（ADR 0046 D5）——
届时 `ControlClient` 的 loopback HTTP 保留为测试适配，而不是生产路径。
