# Design

## Context

见 proposal.md。预检是独立 engine 进程，写 `operator-artifacts/preflight.json`；控制面只读该文件。
首次设置已由宿主持有非机密配置与会话密钥。

## Decisions

1. 预检命令复用 `setup::preflight`：参数取自已保存配置，60 秒期限、有界输出；连通探针仅在显式勾选时启用。
2. 凭据注入与启动一致：变量名取自场景 `provider.credential_env`，密钥不落盘；场景未声明而给密钥则 `HOST_CREDENTIAL_UNUSED`。
3. 结果由**引擎报告的 `all_passed` 与逐轴状态**给出，宿主不重判。报告缺失/进程失败/超时是 `HOST_PREFLIGHT_*` 故障，不与“预检拒绝”混同。
4. 预检不阻塞连接：`not_ready` 如实展示，操作者可选择继续；不把未探测的连通性说成通过。连接后既有面板读取同一份报告。
5. 命令并入 `allow-host-setup`，权限仍是逐条命令的窄集合。

## Risks / Trade-offs

- 演示场景端点是本地占位；未勾选连通探针时 connectivity 恒为 `not_ready`，整体 `all_passed=false`——界面必须如实说明。
- 预检与随后的启动各构造一次 Provider；这不改变预算或凭据边界。
- Windows/设备未验收；本机以真实 engine 进程测试报告产生。
