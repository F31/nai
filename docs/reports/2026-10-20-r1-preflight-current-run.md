# R1 窗口预检：本次运行报告与故障隔离

日期：2026-10-20。工作项：W09/W10、R1 交付顺序 3（D14 仅追溯）。依据 V3.11 §11.5、§12.3、§15、§28.6 与 ADR 0131。
变更已归档：`openspec/changes/archive/2026-10-20-r1-preflight-current-run/`，`desktop-shell` 合入 1 条需求。

## 现在能做什么

重复点击窗口内“运行预检”时，本轮没有报告就明确失败，不会把上一次成功显示成本次通过。
有效的 failed/not_ready 报告继续展示逐轴原因；超时、坏报告和宿主故障与预检拒绝分开。

## 问题与修改

- 原实现运行子进程后直接读固定路径，未清除旧报告；新进程失败却留有旧报告时仍返回旧结果。
- 运行前删除旧报告，删除失败拒绝启动；进程启动失败/超时、报告缺失/损坏等故障清理残留，
  连接后的只读路由不会再读取被失效的旧结论。
- 同一宿主用 try_lock 拒绝重叠预检，不排队，不占控制或静音锁。
- 实际读取最多 256KiB+1 字节，超界拒绝。必要布尔字段、时间字段、非空 checks、唯一非空轴名、
  封闭状态与字符串 reason 缺失或无效时拒绝；退出结果与 all_passed 不符时拒绝。
  不重算业务轴或设备能力，不为缺失值填默认字段。
- 预检故障不回显子进程 stdout/stderr，测试用 secret-sentinel 验证错误消息中没有原始输出。
- 发起窗口预检时清空设置区与连接区的旧结果，失败不会让上一轮“通过”继续留在页面上。
- 无 schema/迁移/依赖变化；Tauri 权限、根 workspace 与 kernel 均未修改。

## 验证记录

使用 WSL Linux 中既有 Rust 1.96.0、Node 22.23.1、pnpm 8.15.0 与项目锁定 OpenSpec 1.14.1。
默认 Windows PATH 的 Node/pnpm 与锁定版本不一致，未替换工具链；切换既有 WSL 环境执行。

- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --offline`：lib **11/11**、
  真实 engine 集成 **3/3**。新增 6 项 lib 测试；既有真实进程用例扩展为连续两次预检和原生控制读回，
  断言连接后的 `report` 与本次落盘 JSON 相等，`connectivity=not_ready`、`playback_ack=false`。
- 独立壳 `cargo clippy ... --all-targets -- -D warnings`、`cargo fmt ... --check` 均通过。
- 独立壳 `cargo build --locked --offline` 与 `timeout 35s xvfb-run -a ... --smoke` 均 EXIT=0，
  输出 `SHELL_PAGE_LOADED`。Xvfb 有 EGL/GTK 环境警告；该证据只验证内嵌页面加载，不代表真实桌面交互。
- 设置 UI 逻辑 **5/5**；变更 `openspec validate ... --strict` 通过。
- 真实控制读回断言第一次错误地把整个响应与报告比较，产生 1 项测试失败；按既有响应封装改为比较
  `response.report` 后复跑 3/3 通过。未为该测试错误修改业务接口。
- 首轮根 `full` 的文档链接判据失败：新报告未进 Git 索引，符合其「只接受已跟踪路径」规则。
  对本轮报告使用 `git add -N` 登记路径（不暂存内容、不提交），该失败在重跑中消除。
- 第二轮 `full` 的 engine 进程集成出现 8 项等待本地 Provider 请求的 Timeout；独立重跑同目标仍为
  11 passed/8 failed。环境存在继承的 HTTP/HTTPS 代理。仅在当前命令中 unset 代理并设置回环
  NO_PROXY 后，同一 `cargo test -p nai-engine --test engine --locked --offline` **19/19** 通过。
  未放宽 deadline、未跳过测试、未改系统代理或产品代码；完整门控在该直连环境再次运行并通过。
- 第三轮 `cargo xtask check --profile full` **EXIT=0**：Rust **1209 passed/0 failed**、
  Node **450 passed/0 failed**。该命令不包含独立壳的 11+3 项，二者分别记录。
- 归档后 `openspec validate --all --strict --no-interactive` **43/43**、
  `openspec validate --archived --no-interactive` **160/160**；定向运行 `spec-merge.test.mjs`、
  `archived-proposal.test.mjs`、`doc-links.test.mjs`，**30/30**。归档只修改规格，因此归档后复核这三项
  与 OpenSpec，没有重复整套编译/测试。
- 全仓 `git diff --check` 报既有生成 C++ 文件的尾随空白；未手工修改生成文件。本轮所改已跟踪文件
  的定向 `git diff --check` 通过。

本次完整门控的回环直连环境（只影响该 shell，非系统配置）：

```bash
unset HTTP_PROXY http_proxy HTTPS_PROXY https_proxy ALL_PROXY all_proxy
export NO_PROXY=127.0.0.1,localhost,::1 no_proxy=127.0.0.1,localhost,::1
cargo xtask check --profile full
```

## 边界与后续

- 单宿主互斥不等于跨进程锁；不要让外部 CLI 与窗口同时向同一数据根写预检报告。
- 故障子进程脚本只用于测试；真实集成另用 nai-engine 进程，均不是付费 Provider 认证。
- Windows 安装/签名、命名管道 ACL/对端认证、设备窗口/音轨输出与硬件 PlaybackAck 仍未完成。
- 本轮未调用付费 API，不调整 registry A12/O01 或其他验收状态。
- 下一产品主线仍为批准候选的独立设备输出与 Windows 安装路径；可用端点、凭据与额度必须当次确认，
  不把历史无余额当作永久阻塞。已有工作区的未提交交付保留，本轮不代为提交。
