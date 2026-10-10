# ADR 0024：工程校验回路的平台事实边界——门控声明范围、具名诊断说明原因、错误类别保持在可移植层

- 日期：2026-10-10
- 状态：Accepted
- 基线：V3.6 §1.4（Slice-0 工程入口）；`docs/engineering.md` 完成证据规则；承接 ADR 0001（workspace 命名）、ADR 0002、ADR 0015（基线与验收注册表）
- 变更：`openspec/changes/engineering-loop-platform-truths/`

## 背景

上一轮报告把 `cargo test --workspace` 的 13 项失败记为「Windows 平台差异」，把 storage 那 4 项写成
「`fs::rename` 覆盖语义与备份/遍历断言」，并记录 profile 在本机无法整体通过。本轮去验证这份解释，
结论是：**其中一项是真缺陷，两项是平台事实但缺少声明方式，一项把「没验证」写成了「通过」**。

- `backup`/`restore` 各自 `File::open(..).sync_all()`：Windows 的 `FlushFileBuffers` 拒绝只读句柄
  （`PermissionDenied(os error 5)`），Linux 对只读 fd 的 `fsync` 合法。**备份与恢复在 Windows 上从未成功过。**
  它之所以像「语义差异」，是因为 `From<std::io::Error>` 把一切压成 `"filesystem operation failed"`。
- 宿主对**任何** stdin 为管道的子进程都拒绝 spawn（`ERROR_PIPE_BUSY`）。同包的 `frame_drill.rs` 已用
  文件级 `#![cfg(target_os = "linux")]` 声明过同类边界，`faults.rs` 没有。
- `0600` 断言是 Unix mode 检查；Windows 上引擎因**别的**原因被拒绝，原断言于是碰巧通过。
- profile 在 clippy 之后跑 `cargo test --workspace`，后者重新链接**正在执行**的 `nai_xtask.exe`，
  Windows 报 `LNK1104`——校验回路在自己身上失败。

## 决策

1. **平台受限的演练由平台事实显式门控**（`#![cfg(target_os = "linux")]`），并**同时**保留具名诊断用于
   「受支持平台仍缺少宿主能力」的残余情形。两者分工明确：门控声明**范围**，诊断说明**原因**。
   因此 `host_stdin_pipe_refusal` 不是与门控矛盾的死代码，而是门控之外的第二道诚实。
2. **不做运行期探测后提前 return，也不用 `#[ignore]`**。libtest 没有 skipped 结局，提前 return 会显示
   `ok`——把「没跑」报告成「通过」，与「不使用 mock、空成功任务为真实认证标 Passed」直接冲突；
   `#[ignore]` 是静态属性，无法表达运行期宿主能力。
3. **只在特定平台机制下成立的规则用特征化断言钉住缺口**，不静默跳过。Windows 的令牌 ACL 检查仍未实现，
   该缺口由一个断言显式承认，将来补上检查时它会立刻失败并要求换成拒绝断言。
4. **持久化确认收敛为单一可移植实现**（`flush_durable`）：以可写方式打开再 `sync_all`，在 Linux 上同样
   正确，因此不做平台分叉；备份与恢复共用同一处，平台知识不得有第二份副本。
5. **错误词表按可移植类别分派**（`io::ErrorKind` 与 SQLite `ErrorCode`），既有的 `ErrorCode::Internal`
   不变，**不**携带 OS 本地化文本。理由：报告必须能区分「被拒访问」与「路径缺失」、区分「库忙」与
   「库损坏」，否则「平台差异」这类误判无法被发现；而 OS 文本是本机语言，不可跨机比对。
6. **harness 拆成 lib + 薄 bin**，profile 的测试步排除自身后再测其 lib：排除的包名取自包自身
   （`env!("CARGO_PKG_NAME")`）而非字面量，避免改名后静默空排除；bin 目标仍由 clippy `--all-targets`
   检查，回避的只是**链接**，检查范围不缩小。
7. **主机的资源限制不写进 portable 检查**。受内存约束的宿主用 cargo 原生开关
   （`CARGO_BUILD_JOBS`）约束并发即可，把它烘进检查等于把一台机器的假设变成项目假设。

## 后果

- `cargo xtask check --profile portable` 在本机**首次整体通过**（EXIT=0，321 passed / 0 failed），
  且备份/恢复路径在这台 Windows 机器上**首次真正可用**——这是产品能力的实质修复，不只是测试转绿。
- 平台边界从此可声明：新增原生子进程演练须先回答「它在哪些平台有意义」，不能默认全平台编译。
- 代价（如实）：`faults.rs` 的 6 项演练在 Windows 上**不编译**，其能力由 Linux 上的执行覆盖，本机未验证；
  `document_expiry` 用例不再覆盖「真实时钟自行走过边界」，改由同文件的 facts 用例以同手法覆盖。
- 残余缺口（不得掩盖）：Windows 令牌 ACL 检查未实现；`schema --check` 与 contracts suite 因本机 node
  无法 spawn 子进程未执行；前端 pnpm 检查未执行；真实设备/GPU/SDK/干净机安装认证未进行。
  本机通过不替代任何产品认证——profile 末行原文即声明这一点。
