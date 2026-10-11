# W44 任务

## 1. 修复诊断脚本的三类缺陷

- [x] 1.1 `STRONG` 的 `error` 由裸子串改为 `\berror(\[|:)`，`thiserror`/`thiserror-impl` 不再命中
- [x] 1.2 新增噪音类：libtest 通过/忽略逐测试行（`test <name> ... ok|ignored`）永不入选
- [x] 1.3 选行改为「强信号 → 后置上下文与所属目标头 → 弱信号」，无信号时才退化尾部窗口，绝不填充配额
- [x] 1.4 目标头前查：每个信号行向前取最近的 `Running …` 行，只带一行

## 2. 截断自证

- [x] 2.1 判据：含逐测试行（`test <name> ... ok|FAILED|ignored`）且不含 `test result:` 汇总行
- [x] 2.2 输出一条 `::warning::`，说明日志在汇总行前结束并给出总行数
- [x] 2.3 完整日志不得出现该警示

## 3. 验证

- [x] 3.1 `STRONG` 正则可判性：12 条判定（`thiserror` 两行须安静；`error[E0308]`/`error: …`/`FAILED`/`panicked` 须命中）
- [x] 3.2 截断样本（复刻本次真实 10 行）⇒ 须输出截断警示 + 目标头 + 失败行，**不含** `thiserror` 两行
- [x] 3.3 完整失败样本 ⇒ 须带出 `failures:` 段、`panicked at …`、断言消息、失败名、`test result: FAILED.`、`error: test failed`
- [x] 3.4 回归 W43 的 2307 行噪声样本 ⇒ 三条真错误（`error[E0599]`、`test result: FAILED.`、`error: could not compile`）仍全部命中
- [x] 3.5 契约不变：日志缺失/为空静默退出 0；无参数退出 2
- [x] 3.6 `node --check scripts/ci-diagnose.mjs` 通过

## 4. 规格、报告与归档

- [x] 4.1 `engineering-foundation` 新增两条需求（并在归档时吸收进主规格）
- [x] 4.2 写 `docs/reports/2026-10-11-w44-ci-diagnose-signal-to-noise.md`（含截断观察与未决点）
- [x] 4.3 跑工程门控与 `validate --all --strict`
- [x] 4.4 `openspec archive -y` 后 `validate --all --strict --archived`
- [x] 4.5 一次 commit + push，并匿名复核该次运行结论
