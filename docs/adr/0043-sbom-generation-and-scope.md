# ADR 0043：SBOM 的生成方式、格式与范围边界

- 状态：接受
- 日期：2026-10-11
- 相关：基线 V3.7 §11.6（插件与供应链）、§11.3 T-08（供应链威胁）、§29.5（构建、产物与发布）、§28（V3.6 补充：供应链审计与 SBOM）、§635 S08；
  [ADR 0041](0041-engineering-profile-frontend-boundary.md)（工程 profile 与前端边界——「portable 只需 Rust 工具链」的既有裁决）、
  `tests/registry.yaml` S08、`xtask/src/secrets.rs`（同族的零新增依赖自建检查）、`xtask/src/deps.rs`

## 背景

基线对供应链有三处明确要求，而仓库里**一处都没有实现**：

- §11.6：「构建：Cargo.lock 与工具链锁定；CI 做依赖漏洞与许可审计；**每次发布附 SBOM**。」
- §29.5：「发布目录含 catalog、兼容矩阵、哈希、签名、**SBOM 与许可**、安装器……」
- §11.3 T-08（供应链：依赖、模型、升级包）：控制含「锁文件；依赖与许可审计；**SBOM**；哈希与签名；撤销检查；受控加载目录」。

`tests/registry.yaml` 的 S08 也自记「SBOM 与真实签名链仍待建」。

「SBOM」一词在本仓库长期是一个**没有定义的空位**：没有规定它由什么生成、什么格式、覆盖什么、不覆盖什么。没有这些规定，
「每次发布附 SBOM」就无法被判真伪——任何一份清单都能自称 SBOM。本 ADR 先解决这一层，再谈实现。

## 证据（本轮实测，先读后判）

**一、`cargo metadata` 已能离线提供 SBOM 所需的绝大部分字段**

```
cargo metadata --format-version 1 --locked --offline   → 1,418,684 bytes
packages = 299      其中 registry 来源 281、workspace 成员 18
含 license 字段 = 299 / 299        仅 license_file 而无 license = 0
含 checksum 字段 = 0 / 299
```

结论：**许可证、名称、版本、来源、仓库地址全部可得，且不需要网络**；只有**校验和拿不到**——当前 cargo 的 metadata 输出
（以及 `cargo_metadata 0.23.1` 的 `Package` 结构）都不含该字段。

**二、校验和在 `Cargo.lock` 里，且与非 workspace 包一一对应**

```
Cargo.lock：[[package]] = 299，checksum = 281
```

281 = 299 − 18，**恰好等于全部 registry/外部包的数目**。即 Cargo.lock 能为每一个需要哈希的包提供 sha256。

**三、工具链现状：一个都没有**

`cargo-deny`、`cargo-audit`、`cargo-sbom`、`cargo-cyclonedx`、`syft`、`cyclonedx-npm` **均未安装**；
仓库内亦无 `deny.toml` / `audit.toml` / 任何 `sbom*` 文件。

**四、xtask 已具备全部所需依赖**

`xtask/Cargo.toml` 现有依赖含 `cargo_metadata = "=0.23.1"` 与 `serde_json = "=1.0.151"`——`check-deps` 正是用前者读真实 metadata。
⇒ **自建生成器不需要新增任何一个依赖。**

**五、许可字符串不全是 SPDX 表达式**

299 个包去重后共 **28 种**许可字符串，其中 **4 种是 SPDX 不接受的旧式斜杠写法**：

```
Apache-2.0 / MIT      Apache-2.0/MIT      MIT/Apache-2.0      Unlicense/MIT
```

SPDX 的运算符只有 `AND`/`OR`/`WITH`，`/` 不是。这 4 种**不能**被当作 SPDX 表达式写入 SBOM。

## 决策

### 决策 1：SBOM 由 xtask 从 `cargo metadata` + `Cargo.lock` 生成，零新增依赖、离线、确定性

新增 `cargo xtask sbom [--out <path>]`。它复用 **`check-deps` 已经在用的同一份 metadata**（`--locked`，指向真实锁文件），
并从 `Cargo.lock` 读取 checksum。不引入任何新依赖、不访问网络。

### 决策 2：输出格式为 CycloneDX 1.5 JSON

CycloneDX 是 cargo 生态的事实标准（`cargo-cyclonedx` 即产出该格式），也是漏洞/合规扫描器普遍摄入的格式。
**只填充能够有据地填充的字段**：`bomFormat`/`specVersion`/`version`/`metadata`/`components[]`（每个组件含 `type`、`bom-ref`、
`name`、`version`、`purl`、`licenses`，registry 包另含 `hashes`）。

**不填充**：`timestamp`、`serialNumber`（决策 4）、依赖关系图 `dependencies`、`vulnerabilities`。

**不声称**：本 ADR 只声称「输出是一份 CycloneDX 1.5 文档」，**不声称**它通过了完整 JSON Schema 校验——
本轮没有引入任何校验器，也没有做外部交叉验证。这一点写进实现报告，不作沉默处理。

### 决策 3：许可只记录，不臆造；不可解析为 SPDX 者原样记为 `name`

分类规则（三态，互斥且穷尽）：

1. 字符串恰为已知 SPDX id ⇒ `{"license": {"id": "<原样>"}}`；
2. 否则，其全部词元（去掉括号、按空白切分）**要么**是已知 SPDX id、**要么**是 `AND`/`OR`/`WITH`，且首尾均非运算符
   ⇒ `{"expression": "<原样>"}`；
3. 否则 ⇒ `{"license": {"name": "<原样>"}}`。

第 3 态**不是失败**，而是**如实记录**：CycloneDX 里 `license.id` 与 `license.name` 是两个不同字段，
所以「这是不是 SPDX 标识」在输出里**天然可见**，不需要额外注解。上面 4 种斜杠旧式写法落在第 3 态。

**明确不做**：不把 `Apache-2.0 / MIT` 静默改写成 `Apache-2.0 OR MIT`。那是替上游做许可解释，属越权；
且斜杠在旧式写法里的含义（并列还是组合）并不由我们裁定。

### 决策 4：确定性优先，省略 `timestamp` 与 `serialNumber`

组件按 `(name, version)` 排序。**同一份锁文件必须产出逐字节相同的输出**——为此放弃 `timestamp`（每次不同）与
`serialNumber`（随机 UUID）。二者在 CycloneDX 中均为可选。

**发布期元数据（时间戳、序列号）注入不在本轮范围**：本仓库尚无发布流水线，构造一个「发布流水线会注入它」的说法
属于未发生的证据。等发布打包真正立项时再定。

### 决策 5：范围边界必须写进产物自身

SBOM 内以 CycloneDX `properties` 记录范围与缺口声明，使产物**自带**其适用边界，离开本仓库也不丢失：

- **覆盖**：锁文件所固定的 Rust 依赖闭包（含 workspace 成员）。
- **不覆盖**：前端 npm 依赖；原生二进制（ORT、CUDA、cuDNN、FFmpeg——§29.5 单独要求记录其版本/许可/分发方式）；
  模型、素材与音色/肖像授权（§11.6 的 model BOM）；插件包自身的 BOM（§29.5 的 plugin-pack）。
- **不能用作**：漏洞审计证据、签名证据、发布验收证据。

### 决策 6：本轮不进工程门控

`cargo xtask sbom` 是**生成命令**，与 `check --secrets` 同类，暂不并入 `engineering_checks`：portable 门控须保持「只需 Rust 工具链」不变
（ADR 0041），且生成会产出文件、不适合塞进只读检查。生成的**单测**仍然在 portable 内运行（`cargo test -p nai-xtask --lib`），
故它**不是**无人使用的幽灵工具。接入 CI 与「发布目录」属后续变更。

## 被否方案与理由

| 方案 | 否决理由 |
| --- | --- |
| `cargo-cyclonedx` | 需 `cargo install`（联网编译）⇒ 引入一个**自身进入供应链**的外部工具，才能产出一份用来审计供应链的清单；而所需字段**已由我们本来就要跑的 metadata 提供**。它产出更完整的字段（含依赖图）——这点记入「未来可选的独立交叉校验器」，不作当前前置。 |
| `cargo-sbom`（EmbarkStudios，SPDX） | 同样需安装；且 cargo 生态扫描器更普遍摄入 CycloneDX。格式选择见决策 2。 |
| `syft`（Anchore） | 非 Rust 二进制，需下载安装 ⇒ 直接违反「portable 只需 Rust 工具链」的既有不变式。 |
| 解析 `cargo tree` 文本 | metadata 已是 `check-deps` 的事实来源；文本解析脆弱且易随 cargo 输出漂移。 |
| 什么都不做 | 设计三处明确要求 SBOM，且 S08 自记待建。空位持续存在，等于把「每次发布附 SBOM」变成不可判真伪的句子。 |

## 影响

- `tests/registry.yaml` 的 **S08 状态不变**（仍 `implemented`）：真实签名链仍缺、漏洞与许可**审计**（非「记录」也非本轮范围）。本 ADR 只消除「SBOM」这一个空位。
- 本 ADR **不**声称满足 §11.6 的「依赖漏洞与许可审计」：那是**政策**（拒绝不合规依赖），需要 advisory 数据源与允许/拒绝清单，
  与「如实记录清单」是两件事。这条仍是开放缺口。
- 与 ADR 0041 一致：portable 的「只需 Rust 工具链」不变。
