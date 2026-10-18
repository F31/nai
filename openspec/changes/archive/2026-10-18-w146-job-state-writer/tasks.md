# Tasks: W146 — 作业状态的写入点、安装器名册与「能轮询 ⇔ 有安装器」

## 1. 勘定

- [x] 量准 15 条在册项里 `kind` 的条目数（**0**）与 `JobView.kind` 的枚举（`[upgrade]`）
- [x] 量准 `INSERT INTO upgrade_journal` 在已跟踪 `.rs`+`.sql` 里的出现次数（**1**，`upgrade.rs:134`）
- [x] 量准 `impl UpgradeHost for` 的实现者（**1**，`RehearsalHost`，演习替身）
- [x] 量准 `stage_upgrade`／`activate_staged` 的调用点（**全部在 `nai-runtime/tests/upgrade.rs`**）
- [x] 订正一次误判：读面**是**接上的（`apps/engine/src/lib.rs:338` `with_jobs`），首次只扫 `crates/` 得出结论为「只在 tests 构造」

## 2. 判据

- [x] `JOB_KINDS[upgrade]` 增加 `writer`（file + text + statement）
- [x] 新增 `WRITE_SCAN_EXTENSIONS = ['.rs', '.sql']` 与 `HOST_IMPL` 逐字形态
- [x] 新增 `JOB_HOSTS` 名册（`RehearsalHost` / `production: false` / `why`）
- [x] `jobSourceErrors` 改具名对象入参（六项全必需），加三条规则
- [x] 修锚点：`pub fn record_upgrade` → `record_upgrade(&mut self`（注入 I2 发现前缀不判别）

## 3. 用例与注入

- [x] 真实仓库用例：写入点恰好一处且就在声明文件里、扫描域非空、无生产安装器
- [x] 合成闭世界用例：先证绿，再逐条破口，**恰好一条**具名错误
- [x] 注入 I1 第二处写入点 → `appears 2 times`（sha256 还原）
- [x] 注入 I2 写入函数改名 → 修锚点前**未红**、修后红（sha256 还原）
- [x] 注入 I3 名册键与实现不符 → 双向各一条（sha256 还原）
- [x] 注入 I4 `production: true` → `no job surface claims a kind`（sha256 还原）
- [x] 注入 I5 登记表一条改成 `kind` → `declares no production host`（sha256 还原）

## 4. 文档

- [x] `docs/adr/0120-job-state-writer.md`
- [x] `docs/reports/2026-10-18-w146-job-state-writer.md`
- [x] `docs/development-plan.md` 追加 W146 段

## 5. 闭环

- [x] `cargo xtask check --profile full`
- [x] `openspec validate --all --strict` 与归档后 `--archived`
- [x] 一次 commit + push + CI 双平台观测
- [x] 回填报告 §六/§七 与 plan 两处占位
