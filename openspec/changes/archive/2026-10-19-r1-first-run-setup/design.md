# Design

## Context

见 proposal.md。壳已拥有 engine 子进程与私有控制通道（ADR 0130）；缺的是“启动前要填什么、填完放在哪、
机密怎么只在本次会话存在”。

## Goals / Non-Goals

**Goals**：双击后由界面完成首次配置、初始化工作区、启动并连接；配置可复用；凭据不落盘。

**Non-Goals**：不实现安装器/签名（Windows 外部条件）；不做系统密钥库持久化（Windows NativeVault 未接线，
非 Windows 明确不可用）；不把数据根选择开放给 WebView；不实现自动启动/自愈重启。

## Decisions

1. 数据根固定为 `app_data_dir/runtime`，由壳解析并只读展示。WebView 不能指定数据根，避免把数据库写到任意路径。
2. 非机密配置 `host.json` 保存在 `app_config_dir`：`schema_version/workspace/actor/profile/scene/runtime_profile`。
   校验标识符、绝对路径与文件存在；Unix 0600、临时文件 + rename。没有机密字段。
3. Provider 凭据由界面输入，仅存于壳内存并作为环境变量注入 engine 子进程；变量名从场景的
   `provider.credential_env` 读取。场景无该字段却输入了凭据 ⇒ 具名拒绝，不静默忽略；输入后立即从界面状态清除。
4. 初始化 = 显式按钮：用 `nai-engine --provision <runtime_profile>`，30 秒期限、有界输出、失败不启动。
   不自动 provision，也不覆盖已有工作区——失败信息原样展示，操作者可改用“直接连接”。
5. 启动 = 显式按钮：`apply_host_setup` 保存配置后派生 engine（`--control-stdio`，可选 `--operator-config`）。
   已在运行时拒绝重复启动（`HOST_ALREADY_RUNNING`），界面改为直接连接。
6. 不自动启动：应用打开只加载设置，不派生 engine；凭据需在连接时输入。托盘退出仍停止本壳启动的 engine。
7. 宿主命令与控制的权限分开：`allow-host-setup` 恰为三条设置命令，`allow-control-exchange` 仍只有一条。

## Risks / Trade-offs

- 环境变量对同用户进程可见（Linux `/proc`、调试器）→ 如实登记；不声称等价于系统密钥库。
- 配置引用绝对路径，移动文件后需重新选择 → 启动/初始化前校验存在并具名失败。
- 初始化不覆盖已有工作区 → 重复点击会显示 provision 的错误，不自动清理数据。
- Windows/macOS 未构建认证；Linux 上以真实 engine 进程测试。
