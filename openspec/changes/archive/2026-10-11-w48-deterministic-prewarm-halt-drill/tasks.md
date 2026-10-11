# W48 任务

## 1. 给被测 drill 一个确定性的 prewarm 阻塞点

- [x] 1.1 `apps/engine/tests/common/mod.rs` 新增 Linux 专用夹具 `park_asset_on_fifo`：把已声明素材换成无写端命名管道
- [x] 1.2 夹具注释写明它成立所依赖的前提（worker 在 prewarm 按序读素材、`File::open` 阻塞）与平台边界

## 2. 让「halt 发生在 prewarm 期间」成为事实

- [x] 2.1 `engine.rs`：把 `/proc` 扫描抽成 `wait_for_supervised_pid`，保留 `wait_for_keeper_pid` 原语义
- [x] 2.2 新增 `wait_for_worker_pid`（`nai-render-worker` + `--render-drill`）
- [x] 2.3 `halt_during_media_drill_prewarm_never_commits_a_frame`：先 park 素材，再等 worker 出现，才写 halt
- [x] 2.4 **断言与决策链校验一字不改**（不放宽断言）

## 3. 验证

- [x] 3.1 `cargo xtask check-deps`
- [x] 3.2 `cargo xtask check --registry`
- [x] 3.3 `cargo xtask check --profile portable`
- [x] 3.4 `openspec validate w48-deterministic-prewarm-halt-drill --strict`
- [x] 3.5 如实披露：被修测试为 Linux-only，本机门控**不能**证明修法成立——
      以 CI ubuntu job 结论为准（`GET /actions/runs` 与 `/check-runs/{id}/annotations` 匿名复核）

## 4. 规格、报告与归档

- [x] 4.1 `engineering-foundation` 新增「演练由构造保证因果顺序」需求（归档时吸收进主规格）
- [x] 4.2 写 `docs/reports/2026-10-11-w48-deterministic-prewarm-halt-drill.md`
- [x] 4.3 更新 `docs/development-plan.md` 与 `AGENTS.md`
- [x] 4.4 `validate --all --strict` → `openspec archive -y` → `validate --all --strict --archived`
- [x] 4.5 一次 commit + push，并匿名复核该次运行的 ubuntu 结论
