# W47 任务

## 1. 工具链选型（ADR 0043）

- [x] 1.1 实测 `cargo metadata --locked --offline` 的可得字段（packages / license / checksum / source）
- [x] 1.2 实测 `Cargo.lock` 的 `checksum` 数目与 registry 包数目是否对应
- [x] 1.3 实测现有工具链（cargo-deny / cargo-audit / cargo-sbom / cargo-cyclonedx / syft）是否已安装
- [x] 1.4 写 `docs/adr/0043-sbom-generation-and-scope.md`，含被否方案与范围边界

## 2. 实现 `cargo xtask sbom`

- [x] 2.1 新增 `xtask/src/sbom.rs`：metadata + 锁文件 → CycloneDX 1.5 JSON
- [x] 2.2 锁文件解析器只读 `name`/`version`/`source`/`checksum`；缺校验和与畸形校验和均具名失败
- [x] 2.3 许可三态分类（SPDX id / SPDX expression / 原样 name）；未声明许可显式记 `NOASSERTION`
- [x] 2.4 组件按 `(name, version)` 排序；省略 `timestamp`/`serialNumber` 以保确定性
- [x] 2.5 `metadata.properties` 写入 scope / excluded / not-evidence-for 三段范围声明
- [x] 2.6 `xtask/src/lib.rs` 加 `pub mod sbom`、`sbom [--out <path>]` 分发与 usage 串

## 3. 测试与非空洞自检

- [x] 3.1 许可分类：单标识符、合法表达式、4 种斜杠旧式写法、畸形表达式
- [x] 3.2 锁文件解析：真实锁文件、缺校验和、畸形校验和、缩进依赖数组不被误读
- [x] 3.3 渲染：每组件必有 licenses 条目、输入顺序无关、哈希与算法正确、范围声明存在
- [x] 3.4 真实产物：生成到文件后与 `Cargo.lock` 逐项交叉核对哈希（281/281）
- [x] 3.5 真实产物：与 `cargo metadata` 逐条核对许可未被改写或丢失（299/299）
- [x] 3.6 确定性：同一锁文件两次生成的字节完全相同
- [x] 3.7 反向自检：把「缺校验和」的锁文件片段喂给解析器时确实失败

## 4. 规格与活文档

- [x] 4.1 新增规格差量 `specs/supply-chain-sbom/spec.md`（归档时吸收为主规格）
- [x] 4.2 写 `docs/reports/2026-10-11-w47-sbom-supply-chain.md`
- [x] 4.3 `docs/engineering.md` 登记 `cargo xtask sbom`
- [x] 4.4 `AGENTS.md` 追加 W47 轮次段
- [x] 4.5 `docs/development-plan.md` 与 `tests/registry.yaml` 的 S08 notes 反映 SBOM 已落地（**status 不动**）

## 5. 验证与归档

- [x] 5.1 `cargo xtask check --registry` 与 `check-deps` 通过
- [x] 5.2 `cargo xtask check --profile portable` 通过
- [x] 5.3 `openspec validate --all --strict` 通过
- [x] 5.4 `openspec archive -y` 后 `validate --all --strict --archived` 通过
- [x] 5.5 一次 commit + push，并匿名复核该次运行结论
