# W47 报告：SBOM 立项——生成方式、格式与范围边界

- 变更：`openspec/changes/w47-sbom-supply-chain`（归档后 `openspec/specs/supply-chain-sbom/spec.md`）
- 裁决：[ADR 0043](../adr/0043-sbom-generation-and-scope.md)
- 实现：`xtask/src/sbom.rs`（新）、`xtask/src/lib.rs`（分发与 usage）
- 产物样例：本轮实测生成 175,111 字节、299 组件的 CycloneDX 1.5 文档

## 1. 缺口不是「少一个文件」，是「SBOM 没有定义」

基线对供应链有三处要求：§11.6「每次发布附 SBOM」、§29.5「发布目录含 … SBOM 与许可 …」、
§11.3 `T-08` 把 SBOM 列为供应链威胁的控制之一。而仓库里的实现是**零**，
`tests/registry.yaml` 的 S08 也只自记「SBOM 与真实签名链仍待建」。

真正的问题是：**「SBOM」在本仓库是一个没有定义的空位**。没有规定它由什么生成、什么格式、
覆盖什么、不覆盖什么。在这个状态下，「每次发布附 SBOM」**不可判真伪**——
任何一份清单都能自称 SBOM，也没有任何东西能证明它漏了什么。

所以本轮的顺序是：**先立规格把「什么才算这份 SBOM」定死，再实现**。ADR 0043 记录裁决，
`supply-chain-sbom` 把裁决写成可判真伪的需求。

## 2. 工具链选型：不引入新工具，用 cargo 自己的视图

先实测四件事，再决定：

| 实测 | 结果 |
| --- | --- |
| `cargo metadata --format-version 1 --locked --offline` | 299 包；**299/299 带 `license`**；281 来自 registry；18 个 workspace 成员；**`checksum` 一个都没有** |
| `Cargo.lock` | `[[package]]` = 299，`checksum` = **281** —— 恰等于非 workspace 包的数目 |
| 已装工具 | `cargo-deny` / `cargo-audit` / `cargo-sbom` / `cargo-cyclonedx` / `syft` **全未安装**；无 `deny.toml`/`audit.toml`/`sbom*` |
| `xtask` 现有依赖 | 已含 `cargo_metadata = "=0.23.1"` 与 `serde_json = "=1.0.151"` —— `check-deps` 正是用前者读真实 metadata |

结论：所需字段（名称、版本、许可、来源、仓库）**已经由我们本来就要跑的那次 metadata 提供**，
只有校验和要另取。因此**不装任何新工具**：为了审计供应链，先引入一个自己进入供应链的外部工具，
是自相矛盾的（`cargo-cyclonedx`/`cargo-sbom` 需 `cargo install` 联网编译；`syft` 是需下载的非 Rust 二进制，
直接违反 ADR 0041 的「portable 只需 Rust 工具链」）。

**零新增依赖**：`Cargo.lock` 未变动，`xtask/Cargo.toml` 未变动。

## 3. 产物长什么样

`cargo xtask sbom [--out <path>]` 输出 CycloneDX 1.5 JSON（不给 `--out` 则写标准输出）。实测：

```
sbom: 299 components — spdx-id 95, spdx-expression 194, verbatim-name 10, no-license 0; 281 carry a sha256
```

每个组件：`type`（有 bin 目标则 `application`，否则 `library`）、`bom-ref` + `purl`（`pkg:cargo/<name>@<version>`）、
`name`、`version`、`licenses`、registry 包另带 `hashes: [{"alg":"SHA-256","content":<锁文件校验和>}]`。
文档级：`bomFormat`/`specVersion`/`version`/`metadata`（component、tools、**三段范围声明**）/`components`。

**只填充能够有据地填充的字段**：不含 `timestamp`、`serialNumber`、依赖关系图、`vulnerabilities`。
本 ADR 只声称「这是一份 CycloneDX 1.5 文档」，**不声称**通过了完整 JSON Schema 校验——
本轮没有引入校验器，也没有做外部交叉验证。这一点如实写在这里，不作沉默处理。

## 4. 三条设计取舍

### 4.1 确定性优先于时间戳

CycloneDX 的 `timestamp` 与 `serialNumber` 都是**可选**的。带上它们，同一份锁文件每次生成都不同；
不带上，**「逐字节相同」成立**，于是两份 SBOM 的 diff 就等价于依赖闭包真的变了。
本轮实测：同一次运行生成两份文件，`cmp` 相同。

代价是没有生成时间。**发布期元数据（时间戳、序列号）注入不在本轮范围**——
本仓库尚无发布流水线，声称「流水线会注入它」属于未发生的证据。

### 4.2 许可分三态，绝不静默改写

工作区 299 个包去重后共 28 种许可字符串，其中 **4 种是 SPDX 不接受的旧式斜杠写法**：
`Apache-2.0 / MIT`、`Apache-2.0/MIT`、`MIT/Apache-2.0`、`Unlicense/MIT`（共 **10** 个包）。

SPDX 的运算符只有 `AND`/`OR`/`WITH`，`/` 不是。所以：

- 塞进 `expression` 字段 ⇒ 产出一份**格式不合法却看似正常**的数据；
- 静默改写成 `Apache-2.0 OR MIT` ⇒ **替上游做许可解释**（斜杠在旧式写法里是并列还是组合，不由我们裁定）。

故分三态：单标识符 ⇒ `license.id`；词元交替且全部为已知 id/运算符 ⇒ `expression`；其余 ⇒ `license.name`（原样）。
**第 3 态不是降级失败**——CycloneDX 里 `id` 与 `name` 本就是两个字段，「这是不是 SPDX 标识」在产物中天然可见。

**「已知 id」是一张显式表而不是形状匹配**：若按「看起来像」判定，`BSD-2-Clause-FreeBSD` 这类 token 会被
当作 `id` 断言，而 SPDX 后来已废弃它、改为 `BSD-2-Clause-Views`。**记成 `name` 是少说，断成 `id` 可能说错**。
代价是表外的合法 id 会被记成 `name`——所以生成时把四类计数打到 stderr（见 §3 那行），让这种偏移**可见**而非无声。

### 4.3 范围声明写进产物本身

物料清单最容易被误读成安全结论：它列了 299 个组件，看起来「清点过了就没问题」。但它**不含**漏洞信息、不含签名、
不含前端 npm 依赖、不含 ORT/CUDA/FFmpeg 等原生二进制、不含模型与素材、也不含插件包自身的 BOM。

这些写进 `metadata.properties`（`nai:sbom:scope` / `nai:sbom:excluded` / `nai:sbom:not-evidence-for`），
是为了让产物**离开本仓库后仍自述边界**——用仓库内文档去约束一份会外发的产物，等于没有约束。

## 5. 验证

### 5.1 与两个独立来源逐项交叉核对

| 核对 | 结果 |
| --- | --- |
| 组件数 vs 锁文件 `[[package]]` | 299 = 299 |
| 哈希 vs `Cargo.lock`（逐个） | **281/281 一致，0 处不符** |
| 许可 vs `cargo metadata`（逐条） | **299/299 原样一致，0 处改写或丢失** |
| 斜杠旧式写法的包数 vs `verbatim-name` 计数 | 10 = 10 |
| `purl` 唯一性 / 每组件含 `bom-ref`+`purl` | 通过 |

哈希与许可这两项是**用独立的 Python 实现**从两侧重新解析后比对的，不是复用被测代码的结论。

### 5.2 单测与非空洞自检

`cargo test -p nai-xtask --lib`：**40 → 51 passed / 0 failed**（新增 11 项）。覆盖：
许可三态（含 4 种斜杠反例与 `MIT MIT`、`OR MIT`、`MIT OR` 等畸形）、锁文件解析（真实锁文件、缺校验和、畸形校验和、
缩进依赖数组不被误读）、渲染（每组件必有 `licenses`、输入顺序无关、哈希算法、范围声明存在）。

**反向自检**（确认守卫不是空的）：把一段**缺 `checksum` 的 registry 包**喂给解析器 ⇒ 报
`… comes from a registry but has no checksum`；把畸形校验和喂进去 ⇒ 报 `… not 64 lowercase hex digits`。
两条都在测试里钉住。

**一次真实的失败暴露了一个 bug**：解析器最初在**匹配键之前**就解引号，于是 `dependencies = [` 被当作
「不是带引号的字符串」而报错——真实锁文件与夹具测试同时失败。修法是**先匹配键、再解引号**。
这是本轮唯一一处靠测试失败才发现的缺陷，记在这里以免它看起来像是「一次写对」。

### 5.3 交叉核对守卫本身

生成时逐项核对：锁文件里每个带校验和的包，都必须在 metadata 里找到对应组件，否则**具名失败**，
而不是产出一份静默少一个包的物料清单。

## 6. 明确不做

- **不改任何验收 `status`**：S08 仍为 `implemented`。本变更只消除「SBOM」这一个空位。
- **不做漏洞审计**：那需要 advisory 数据源与允许/拒绝清单（`cargo-audit`/`cargo-deny` 一类），
  且是**政策**（拒绝不合规依赖）而非**记录**。这条仍是开放缺口。
- **不并入 `engineering_checks`**：生成是产出命令（会写文件），不是只读检查。portable 语义不变
  （ADR 0041）。但其**单测在门控内**——`cargo test -p nai-xtask --lib` 覆盖 `sbom.rs`，
  故它不是无人使用的幽灵工具。接入 CI 与「发布目录」属后续变更（与 W41 落门控、W42 再接 CI 的次序一致）。
- **不覆盖**：前端 npm 依赖、原生二进制、模型与素材、插件包 BOM。已在产物内声明。
- **不做发布期元数据注入**（时间戳、序列号）。
- **不做外部 Schema 校验**。

## 7. 与 S08 的关系

S08 的验收行是「插件与供应链：篡改包拒载；撤销生效；清单自报信任被忽略；未信任插件不能以 inproc 加载」——
这几条此前已实现。S08 的 notes 里另记两项待建：**SBOM** 与**真实签名链**。
本轮只解决前者，后者仍缺，**所以 S08 不升级**。
