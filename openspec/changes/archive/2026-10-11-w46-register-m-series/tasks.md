# W46 任务

## 1. 扩展 id 家族至 M

- [x] 1.1 `scripts/contracts/registry.mjs` 的 `ID_RE` 改为 `^(S|R|A|B|O|P|M|X)[0-9]{2}$`
- [x] 1.2 `schemas/tests/registry.schema.json` 两处 pattern（acceptance.id 与 mapping targets）同步加入 `M`
- [x] 1.3 `tests/registry.test.mjs` 新增家族守卫（M01–M18 全部在册）并上调条目下限（44 → 66）

## 2. 登记 M01–M18

- [x] 2.1 按 §37.5（M01–M12）与 §38.10（M13–M18）逐条填 id/title/release_line
- [x] 2.2 全部 `status: pending`（在册未开始）；`automation` 标为规划意图并在 notes 写明
- [x] 2.3 `contract` 仅在 §37.4 命名了契约时填（M10/M12 留空）
- [x] 2.4 `mapping.notes` 补一句：M 家族不在 D01–D14 映射内（源自 W26–W40）

## 3. 同步活文档的家族枚举

- [x] 3.1 `AGENTS.md`（含 id 正则那处）→ `S/R/A/B/O/P/M/X`
- [x] 3.2 `docs/design.md`、`docs/development-plan.md`、`openspec/config.yaml`、`README.md` 同步

## 4. 验证

- [x] 4.1 `cargo xtask check --registry` 通过，家族分布 = S12/R11/A11/B6/O5/P1/M18/X2（共 66）
- [x] 4.2 既有 48 条的 status 一字未动（implemented 27 / tested 10 / blocked 5 / not_applicable 3）
- [x] 4.3 家族守卫非空洞：临时移除一条 M 条目时测试确实失败（反向自检）
- [x] 4.4 负例：旧正则（不含 M）下登记 M01 会失败——证明扩白名单是必要的
- [x] 4.5 `openspec validate --all --strict` 通过

## 5. 规格、报告与归档

- [x] 5.1 `admission-registry` 新增一条需求（归档时吸收进主规格）
- [x] 5.2 写 `docs/reports/2026-10-11-w46-register-m-series.md`
- [x] 5.3 `cargo xtask check --profile portable` 通过
- [x] 5.4 `openspec archive -y` 后 `validate --all --strict --archived`
- [x] 5.5 一次 commit + push，并匿名复核该次运行结论
