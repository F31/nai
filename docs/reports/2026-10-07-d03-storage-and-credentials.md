# D03 存储与系统凭据边界

## 状态与范围

- OpenSpec：`d03-storage-and-credentials`；关联 ADR 0004。
- 已实现：nai-storage/nai-security、34 表/两步迁移、单 owner、配置/事实权限与版本、审批失效、在线备份恢复、系统凭据引用/broker。
- Linux/WSL 实测通过：12 项存储测试 + 4 项安全测试；全部 workspace 32 项 Rust 测试通过。
- Windows GNU 的 nai-security 及测试代码交叉检查通过；不是原生执行或 MSVC 安装认证。
- Windows Credential Manager 真实读写：**Blocked，当前无 Windows 执行环境**；测试已配置在 Windows runner 中真实执行。
- IPC/原生宿主隔离、媒体静音/outbox 消费者、实际 RAG/Provider 及恢复后 engine preflight 未实现，仍对应后续 D04–D19。
- 本轮未提交/推送，未调用付费 API，未保存生产凭据或观众数据。

## 修改目录

`migrations/sqlite`、`crates/nai-storage`、`crates/nai-security`、
`schemas/config/backup.schema.json`、生成 TS backup 类型、测试/xtask/CI，
README/AGENTS、工程/存储指南、开发计划、ADR 0004 与 OpenSpec 工件。

## 关键行为

- 连接私有，OS 文件锁拒绝第二 owner；WAL/FULL、外键、50ms busy timeout、进度回调与取消/期限。
- 检查迁移版本/checksum/实际 DDL，错误迁移事务回滚，不仅信任 migrations 表。
- 复用生成 Fact/Approval/Cue，Money/decimal 不转浮点；CAS 修订、不可改历史与 workspace/actor 过滤。
- 原权限撤销或事实更新事务内撤销旧审批/候选/缓存有效状态及写入 outbox；事实到期复检阻止存储层认可旧审批。
- 配置读写以同源离线 Rust JSON Schema 校验，拒绝明文 key 字段和携凭据 URL。
- 原文/许可/素材或模型清单经登记的不可变文件纳入备份；数量/大小/路径受限。
- 在线备份包含 WAL 中提交，清单记录数据库/文件哈希、schema/版本组合及敏感范围。
- 恢复仅到不存在的新目录，原数据不覆盖；旧会话提高 epoch/停机，旧授权、审批、候选、缓存与设备撤销。
- 凭据仅 Windows 系统存储；非 Windows 显式 Unavailable；mock 仅测试。读取逐次复检 broker 当前授权。
- Secret 缓冲使用 Zeroizing，Debug/错误不包含密钥。软件 broker 不声称隔离同用户恶意原生进程。

## 锁定环境

Rust 1.96.0、Node 22.23.1、pnpm 8.15.0、OpenSpec 1.14.1。
rusqlite 0.40.2（bundled/backup/hooks，默认 features 关闭）、sha2 0.11.0、directories 6.0.0、
jsonschema 0.58.6（HTTP/file resolve 关闭）、zeroize 1.9.1、keyring 3.6.3 windows-native。
依赖由 Cargo 实际生成锁文件；SQLite 编译原生库，无独立 DB 服务。

## 实际运行与结果

| 命令 | 结果 |
|---|---|
| `cargo generate-lockfile`、`cargo xtask schema` | 新依赖锁定，backup TS 类型生成成功 |
| `cargo xtask schema --check` | 无漂移 |
| `cargo xtask test --suite storage` | 12 存储 + 4 安全测试通过 |
| `cargo xtask check --profile portable` | 6 包依赖边界、fmt、clippy、32 项 Rust 测试通过 |
| `cargo xtask test --suite contracts` | 8 项 Node 校验、Rust/TS/C++ 编译及两条互通路径通过 |
| `pnpm typecheck`、`pnpm build` | desktop/contracts 均通过 |
| `OPENSPEC_TELEMETRY=0 pnpm spec:validate` | D03 变更及已有规格严格校验通过 |
| `rustup target add x86_64-pc-windows-gnu --toolchain 1.96.0` | 首次下载超过300s工具时限；未记通过 |
| 指定官方 `RUSTUP_DIST_SERVER=https://static.rust-lang.org` 重试上述命令 | 安装成功 |
| `cargo check --locked -p nai-security --tests --target x86_64-pc-windows-gnu` | Windows 分支/真实测试代码检查通过；未执行 |
| `pnpm exec openspec archive d03-storage-and-credentials --yes`（遥测关闭） | 10/10 任务完成，7条需求合并2份主规格，归档成功 |
| 归档后 `pnpm spec:validate`、`openspec validate --archived --no-interactive`（遥测关闭） | 5份主规格与D01–D03三项归档全部通过 |

归档：[D03 proposal/specs/design/tasks](../../openspec/changes/archive/2026-10-07-d03-storage-and-credentials/)。
新增主规格：`openspec/specs/local-persistence/spec.md`、`openspec/specs/credential-boundary/spec.md`。

## 测试证据

12 项存储测试覆盖：

1. 完整表目录、FK、WAL/FULL 与默认无长期观众记忆。
2. 第二 owner、已取消/过期调用及 SQLite 真实长查询进度中断。
3. 迁移半步失败回滚、checksum/未来版本/无版本旧表/实际 DDL 漂移拒绝。
4. 精确事实、CAS 冲突、越权/跨空间拒绝、不可变历史。
5. 候选文本 hash、实际批准 actor、事实更新/权限撤销同事务失效。
6. 事实到期阻止已有与新审批，不依赖 flaky sleep。
7. 同源配置、版本冲突、未知 plaintext key 字段和 credential URL 拒绝。
8. 有提交 WAL 的备份，原文/配置/事实恢复和旧设备/权限/审批撤销。
9. 新演练会话记录、保留审计，源后备份的新事实不被恢复覆盖。
10. 损坏 DB/内容/hash/穿越 manifest 及 symlink 拒绝，失败不激活目标。

这些为多个断言合并的12项测试，不是12个产品 T/E/U 编号。
4项 Linux 安全测试覆盖引用格式/脱敏、权限和撤销前置、期限取消前置及生产后端 Unavailable。
Windows 原生 test 仅会创建唯一 test namespace 条目、读写并清理，当前未运行。

## 失败与修复

- 新版 rusqlite progress_handler 返回 Result；实际编译警告发现未传播错误，已用 `?` 处理。
- clippy 发现配置写接口参数过多，改为 workspace/actor 作用域 tuple；检查重新通过。
- Windows Rust target 下载超时；官方源访问核查与重试后交叉编译成功，原生认证仍 Blocked。
- 媒体不存在时 session read 使用 MediaUnspecified，不把数据恢复写成“已实际静音”。

## 迁移、维护与成本边界

首次 schema 1→2，无旧产品数据迁移。现阶段备份只认可当前 runtime/schema/catalog 组合，
未来组合兼容列表和维护窗口升级归 D17；不承诺无限回退或接受未知来源备份。
新增开发/原生依赖，无常驻服务、云资源、模型下载或 API 使用。
当前原文/清单文件须登记，未登记/大权重/索引不宣称已备份。
同步 OS 操作不能任意抢占，P0 需独立链路；本轮未测Windows设备/ACL、GPU或长播性能。

下一项：**D04 内核注册与绑定**，独立 OpenSpec 变更验证依赖闭包、能力视图与 ReadyDegraded/NotReady。
