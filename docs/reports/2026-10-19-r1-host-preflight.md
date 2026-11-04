# R1 宿主预检：界面内产生并展示一键预检报告

日期：2026-10-19。变更已归档至 `openspec/changes/archive/2026-10-20-r1-host-preflight/`。依据：
[ADR 0131](../adr/0131-first-run-setup-boundary.md)、[ADR 0130](../adr/0130-native-control-inherited-pipe.md)。

## 现在能做什么

- 首次设置面板新增“运行预检”：用已保存设置运行 `nai-engine --preflight`，逐轴展示
  passed/failed/not_ready 与原因；可选连通探针（使用本次会话 API 密钥）。
- 报告写到数据根 `operator-artifacts/preflight.json`；连接后既有“演示包一键预检”面板读取同一份。
- 预检不阻塞连接；`not_ready` 不会被说成通过。
- 进程无法启动、超时或未写出报告分别返回 `HOST_PREFLIGHT_SPAWN_FAILED`/`HOST_PREFLIGHT_TIMEOUT`/
  `HOST_PREFLIGHT_REPORT`，与“预检拒绝”区分。

## 实现

- `setup::preflight`：60 秒期限、有界输出、可选 `--preflight-connectivity`；凭据名仍取自场景
  `provider.credential_env`，密钥只进入本次子进程环境。
- 宿主命令 `run_host_preflight` 并入 `allow-host-setup`；命令清单与 TOML 仍逐条核对。
- 结果直接取引擎报告的 `all_passed` 与 `checks`，宿主不重算；报告缺失是故障，不是通过。
- 桌面 `preflightRunSummary` 与逐轴列表；提交后清空密钥状态。

## 实测

- 壳独立 workspace：lib **5** + 真实 engine 集成 **3**（其中首次运行用例追加：预检报告存在、
  scene=passed、connectivity=not_ready、`all_passed=false`）；clippy 干净；`--smoke` Xvfb **EXIT=0**。
- `tests/ui/setup.test.mjs` **5/5**；`desktop-shell.test.mjs` **16/16**；`native-ipc.test.mjs` **6/6**。
- 平台矩阵 22/22；`check-deps` 20 包。

- `cargo xtask check --profile full` **EXIT=0**：**105 个测试目标**（86 `Running` + 19 `Doc-tests`）、
  **1209 passed / 0 failed**；Node **450 项 / 0 失败**（较上轮 449 增加预检摘要用例）。
  壳的独立测试、clippy 与 `--smoke` 均单独执行。归档后复跑同一命令仍 **EXIT=0**、计数逐项相同；
  `openspec validate --all --strict` **43/43**、`--archived` **159/159** 通过。

## 未实现与未验收

- 演示场景端点是本地占位；不勾选连通探针时 connectivity 恒为 `not_ready`，整体不会显示通过。
- 预检不替代真实 Provider 认证或设备输出；`playback_ack=false`。
- Windows 安装/托盘/签名与设备验收未执行；A12/O01 状态不变。
