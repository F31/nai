# 任务：w52 开放契约册与门控

> 勾选前先跑对应验证；未验证不得勾。

## 0. 回归修复（前置）

- [x] 0.1 `tests/contracts/schema.test.mjs` 的 backup 样本 `db_schema_version` 由 5 改为 6，并**补一条
      负样本**钉住「上一个版本号必须被拒绝」；本地 `node --test tests/contracts/schema.test.mjs` 11/11 通过
      （修复 W51 造成的 CI 步骤 19 双平台红）

## 1. 能力目录阶段字段与六条 Draft

- [x] 1.1 `schemas/catalog/catalog.schema.json`：条目 `required` 增 `stage`；条件约束——`frozen` 须有
      `request`/`response`/`stream`，`draft` 禁有这四个字段；`minItems` 27→33
- [x] 1.2 `schemas/catalog/capabilities.json`：现有 27 条标 `"stage":"frozen"`；新增六条 `draft`——
      `data.query@1`、`data.subscribe@1`、`device.sense@1`、`device.control@1`、`asr.transcribe@1`、
      `market.catalog@1`（依 §37.4 与第 63/149/151 行）
- [x] 1.3 `scripts/contracts/validation.mjs` 的 catalog 语义检查：adapter 目标收紧为**必须 frozen**；
      复述 draft 禁声明消费者字段

## 2. 生成器区分阶段

- [x] 2.1 `xtask/src/schema.rs::catalog_types`：`frozen` 才解析 `request`/`response`；
      `draft` 声明了消费者字段即报错；单测补 draft 半声明与未知 stage 两个负样本
- [x] 2.2 `xtask/src/schema.rs::generate`：`session_types` 与 `capability_ids` 只取 `frozen`；
      新增 `DRAFT_CAPABILITY_IDS` 写入 `generated/catalog.rs`；打印计数区分 frozen/draft
- [x] 2.3 重新生成三份产物（本机需临时异步 shim，跑完**已还原** `scripts/contracts/generate.mjs`）

## 3. 门控：契约可解析

- [x] 3.1 `scripts/contracts/registry.mjs`：导出读目录 id 的能力；`validateRegistry` 校验每条 `contract`
      可解析（目录 id 或非能力白名单 `plugin-hosts-and-ipc`，附理由）；不传目录即错
- [x] 3.2 `tests/registry.test.mjs`：传入目录 id；新增负样本（悬空 `market.catalog@9` 被指名拒绝）与
      正样本（draft 契约可被引用且**不使 status 升级**）
- [x] 3.3 闭门控盲区：拆出 `tests/contracts/descriptor.test.mjs`（依赖 `target/contracts/descriptor.bin`），
      `engineering_checks` 运行静态的 `tests/contracts/schema.test.mjs`，CI 步骤 19 与 contracts suite
      运行两者

## 4. 契约测试同步

- [x] 4.1 `tests/contracts/schema.test.mjs`：目录用例改为不依赖描述符（唯一 id、stage 合法、draft 集合、
      两处拒绝）；`descriptors`/`readFile` 导入随之移除
- [x] 4.2 `tests/contracts/descriptor.test.mjs`：断言 frozen 的 request/response 均可解析且 `stream` 为布尔；
      断言 draft 不声明任何消费者字段

## 5. 文档、勘误与报告

- [x] 5.1 `docs/contracts/README.md`：能力计数改为「27 Frozen + 6 Draft」，写明阶段语义与门控分工
- [x] 5.2 `docs/reports/2026-10-11-w51-approval-final-text.md` **追加勘误段**（不改写原文）：
      W51 的 CI 实为 `failure`（两平台步骤 19），根因是样本未同步；并更正「CI 全绿」的观测
- [x] 5.3 `docs/development-plan.md`、`AGENTS.md` 同步 W52
- [x] 5.4 新建 `docs/reports/2026-10-11-w52-open-contract-catalog.md`

## 6. 验证与闭环

- [x] 6.1 `check-deps`、`check --registry`、`node --test tests/registry.test.mjs`
- [x] 6.2 `cargo xtask schema`（临时 shim）→ `node --test tests/contracts/schema.test.mjs` 11/11、
      `descriptor.test.mjs` 2/2、`tsc -p packages/contracts-ts` EXIT=0
- [x] 6.3 `check --profile full`（与 CI 同命令）全量：**EXIT=0**
- [x] 6.4 `tests/ui` 37/37（含在前端门控内）
- [x] 6.5 `openspec validate w52-open-contract-catalog --strict` → `validate --all --strict`
- [x] 6.6 `openspec archive -y` → `validate --all --strict --archived`
- [x] 6.7 一次 commit + push；用**缓存破坏参数**匿名复核 CI：断言 `head_sha` 与推送一致、
      记录 conclusion 与失败步骤
