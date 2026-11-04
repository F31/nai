# ADR 0131：首次设置与非机密配置边界

- 状态：接受
- 日期：2026-10-19
- 依据：V3.11 §11、§29、§900；ADR 0046、ADR 0129、ADR 0130

## 背景

原生壳已能经命令行参数启动 engine（ADR 0130），但「安装后无需命令行配置」仍未满足：
数据根、工作区、场景和 Provider 凭据都只能从终端传入，双击应用无法完成首次配置。

## 决策

1. **数据根固定为应用数据目录下的 `runtime/`**，由壳解析。WebView 只读展示，不能指定数据根，
   避免把数据库写到任意路径。
2. **非机密配置落盘**：`app_config_dir/host.json`，字段为工作区、身份、profile、场景路径与
   runtime profile 路径；标识符与绝对路径校验，Unix 0600、父目录 0700、临时文件 + rename。
   文件中没有凭据字段。
3. **凭据只用于本次会话**：界面输入的 API 密钥不写入文件、日志、命令行或 WebView 持久状态；
   壳从场景的 `provider.credential_env` 读取变量名，把它注入自己派生的 engine 子进程。
   场景未声明该字段却输入密钥 ⇒ `HOST_CREDENTIAL_UNUSED`，不静默忽略。
4. **初始化与启动都是显式动作**：初始化调用 `nai-engine --provision`（30 秒期限、有界输出），
   失败不启动；启动派生 `--control-stdio` engine 并可带 `--operator-config`。已运行时返回
   `HOST_ALREADY_RUNNING`，不创建第二个进程。应用启动不自动派生 engine。
5. **设置不是第二套业务面**：它只保存主机配置、运行 provision 与启动；会话、审批、知识、守门和
   媒体判定仍由同一个 `ControlApi` 与 engine 执行（ADR 0130 不变）。
6. **权限分面**：`allow-host-setup` 恰覆盖 `host_defaults`/`host_setup`/`apply_host_setup`/
   `run_host_preflight`，`allow-control-exchange` 仍只有 `control_exchange`；测试从 `permissions/*.toml`
   派生并逐条核对。
7. **预检是显式宿主动作，不是启动门禁**：`run_host_preflight` 用已保存设置运行 engine 预检，结果直接取
   引擎报告的 `all_passed`/逐轴状态；宿主不重判，也不把 `not_ready` 说成通过。进程失败/超时/缺报告是
   `HOST_PREFLIGHT_*` 故障，与“预检拒绝”分开。报告写入数据根，连接后由既有只读路由读回。

## 成本与边界

- 壳的独立 workspace 新增 `serde`/`serde_json`/`zeroize`（复用根锁定版本）；无常驻服务，
  根 workspace 与内核依赖方向不变。
- 环境变量对同用户进程/调试器可见（Linux `/proc`）；这不是系统密钥库。Windows `NativeVault`
  未接线，非 Windows 明确不可用，因此本轮不声称「安全保存凭据」。
- 数据根固定意味着暂不支持迁移已有数据根；迁移属部署生命周期（ADR 0027），不在本轮。
- 初始化不会覆盖已有工作区；重复初始化的错误原样展示。场景/runtime profile 移动后需重新选择。
- Windows/macOS 构建、托盘、安装器与签名仍未验收；Linux 以真实 engine 进程测试。
