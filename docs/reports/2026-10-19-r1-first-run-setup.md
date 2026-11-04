# R1 首次设置：无命令行配置、会话凭据与初始化

日期：2026-10-19。变更已归档至 `openspec/changes/archive/2026-10-19-r1-first-run-setup/`。依据：
[ADR 0131](../adr/0131-first-run-setup-boundary.md)、
[ADR 0130](../adr/0130-native-control-inherited-pipe.md)。

## 现在能做什么

- 双击原生壳后，在界面中填写工作区、身份、profile、场景与 runtime profile；数据根由宿主固定并只读展示。
- 勾选“首次使用，初始化工作区”后一次完成：保存非机密设置 → `nai-engine --provision` → 派生 engine → 连接。
- API 密钥只在本次会话输入，作为环境变量进入本壳派生的 engine；关闭应用后需重新输入。
- 配置写入 0600 的 `host.json`，不含密钥；重复启动被 `HOST_ALREADY_RUNNING` 拒绝，不产生第二个 engine。
- 设置说明与错误来自真实状态：未保存/已保存/运行中/命令行来源/provision 失败分别呈现。

## 实现

- `apps/desktop/src-tauri/src/setup.rs`：非机密配置校验、0600 原子写、场景 `provider.credential_env`
  解析、30 秒有界 provision、默认值与有效路径预填。
- 壳状态新增 `host_defaults`/`host_setup`/`apply_host_setup` 三条命令；设置与引擎启动在同一宿主内，
  WebView 仍不能命名数据根、进程或凭据文件。`allow-host-setup` 的 `commands.allow` 由测试逐条核对。
- 桌面 `setup.ts` + 面板：表单校验、初始化复选框、密码框与“启动并连接”；提交后立即清空密钥状态。
  非原生浏览器继续使用显式 loopback 测试适配。
- 演示包新增 schema 合法的 `runtime.json`；`workspace_id` 与表单工作区不一致时宿主在派生进程前具名拒绝。

## 实测

- 壳独立 workspace：lib **5** + 真实 engine 集成 **3** 全绿（首次运行 provision/连接/状态、重复启动、
  非法标识符/缺文件/未使用密钥、未初始化数据根失败且无进程）。
- 壳 `clippy --all-targets -- -D warnings` 干净；`--smoke` 在 Xvfb **EXIT=0**。
- `tests/ui/setup.test.mjs` **4/4**；`desktop-shell.test.mjs` **16/16**；`native-ipc.test.mjs` **6/6**。
- `check-deps` 20 包；平台矩阵 22/22（新宿主的 0600/rename 分支已登记）。

- `cargo xtask check --profile full` **EXIT=0**：**105 个测试目标**（86 `Running` + 19 `Doc-tests`）、
  **1209 passed / 0 failed**；Node **449 项 / 0 失败**（较上轮 445 增加本轮 4 项设置 UI）。
  壳的独立测试、clippy 与 `--smoke` 均单独执行。归档后复跑同一命令仍 **EXIT=0**、计数逐项相同；
  `openspec validate --all --strict` **43/43**、`--archived` **158/158** 通过。

## 未实现与未验收

- 凭据不是系统密钥库保存：环境变量对同用户进程可见；Windows NativeVault 未接线，重启需重输。
- 数据根固定，尚无迁移/选择已有数据根；场景与 runtime profile 仍是绝对路径输入。
- provision 不覆盖已有工作区；重复初始化会显示 engine 的错误，不自动清理数据。
- Windows 安装/签名/托盘与设备输出未验收；没有 PlaybackAck。A12/O01 状态不变。
