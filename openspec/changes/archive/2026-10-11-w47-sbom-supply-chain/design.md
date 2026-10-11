# 设计

## 1. 为什么用 cargo 自己的 metadata，而不是装一个 SBOM 工具

候选与实测：

| 方案 | 为什么否 |
| --- | --- |
| `cargo-cyclonedx` | 需 `cargo install`（联网编译）⇒ 为了审计供应链，先引入一个**自己进入供应链**的外部工具。而所需字段（name/version/license/source/repository）**已由我们本来就要跑的 metadata 提供**。 |
| `cargo-sbom`（EmbarkStudios，SPDX） | 同样需安装；且 cargo 生态扫描器更普遍摄入 CycloneDX。 |
| `syft`（Anchore） | 非 Rust 二进制、需下载 ⇒ 直接违反 ADR 0041 的「portable 只需 Rust 工具链」。 |
| 解析 `cargo tree` 文本 | metadata 已是 `check-deps` 的事实来源；文本解析脆弱且随 cargo 输出漂移。 |
| 什么都不做 | 设计三处要求 SBOM；空位持续存在，等于把「每次发布附 SBOM」变成不可判真伪的句子。 |

**实测依据**：`cargo metadata --format-version 1 --locked --offline` 返回 299 个包，
**299/299 带 `license`**、281 个来自 registry、18 个 workspace 成员；全部字段离线可得。
唯一拿不到的是 `checksum`（当前 cargo 的 metadata 输出与 `cargo_metadata 0.23.1` 的 `Package` 结构都没有该字段），
故哈希单独从 `Cargo.lock` 取——那里有 **281** 条 `checksum`，恰好等于非 workspace 包的数目。

## 2. 为什么哈希必须来自锁文件，且缺了就报错

`Cargo.lock` 的 `checksum` 是 sha256，正好是 CycloneDX 的 `SHA-256`。
一个来自 registry 的包若在锁文件里没有校验和，那是**锁文件形态异常**，不是正常情况——
此时**报错优于跳过**：跳过会产出「看起来完整、实则少一个哈希」的物料清单，
而这类产物恰恰是要被用来核对供应链的。

解析器因此刻意做得极小：只从每个 `[[package]]` 块读 `name`/`version`/`source`/`checksum` 四个键。
**先匹配键、再解引号**——因为块里还有 `dependencies = [ ... ]`，它的值不是带引号的字符串
（这一条是测试先失败才发现的，见报告）。值不是带引号字符串、校验和不是 64 位小写十六进制，都是**具名错误**。

## 3. 为什么许可要分三态，而不是一律当 SPDX 表达式

工作区 299 个包去重后共 **28 种**许可字符串，其中 **4 种是 SPDX 不接受的旧式斜杠写法**：

```
Apache-2.0 / MIT      Apache-2.0/MIT      MIT/Apache-2.0      Unlicense/MIT
```

SPDX 的运算符只有 `AND`/`OR`/`WITH`，**`/` 不是**。把这 4 种塞进 CycloneDX 的 `expression` 字段，
就是产出一份**格式不合法却看似正常**的数据；把它们静默改写成 `Apache-2.0 OR MIT`，
则是**替上游做许可解释**（斜杠在旧式写法里是并列还是组合，并不由我们裁定）。

所以分三态：

1. 恰为已知 SPDX id ⇒ `{"license": {"id": ...}}`；
2. 词元交替（标识符/运算符）且全部为已知 id 与运算符 ⇒ `{"expression": ...}`；
3. 否则 ⇒ `{"license": {"name": <原样>}}`。

第 3 态**不是降级失败**：CycloneDX 里 `license.id` 与 `license.name` 本就是两个不同字段，
所以「这是不是 SPDX 标识」在产物中**天然可见**，不需要额外注解。实测 299 个包中
`id 95 / expression 194 / name 10`，而斜杠旧式写法恰好覆盖 **10** 个包——两者吻合。

**「已知 id」是一张显式表，而不是形状匹配**。若按「看起来像标识符」判定，
`BSD-2-Clause-FreeBSD` 这类 token 会被当作 `id` 断言——而 SPDX 后来已把它废弃改为 `BSD-2-Clause-Views`。
记成 `name` 是**少说**，断成 `id` 可能**说错**。少说优先。代价是未来某依赖若用了表外的合法 id，
它会被记成 `name`——所以生成时把四类计数打到 stderr，让这种偏移**可见**而不是无声。

## 4. 为什么省略 `timestamp` 与 `serialNumber`

两者在 CycloneDX 中都是**可选**的。带上它们，同一份锁文件每次生成的字节都不同；
不带上，「同一锁文件 ⇒ 逐字节相同」成立，于是**两份 SBOM 的 diff 就等价于依赖闭包真的变了**。
这比「每次生成都不同、只能靠人看」有用得多。代价是没有生成时间——**发布期元数据（时间戳、序列号）
注入不在本轮范围**：本仓库尚无发布流水线，声称「流水线会注入」属于未发生的证据。

## 5. 为什么生成命令不进 portable 门控

ADR 0041 定的不变式是「`portable` 只需 Rust 工具链」。SBOM 生成与 `check --secrets` 同类：
它是**产出**命令，不是**检查**命令，且会写文件，塞进只读检查不合语义。
故 `engineering_checks` 一行不改，`portable` 语义不变。

但**它的单测在 portable 内**——`cargo test -p nai-xtask --lib` 覆盖 `sbom.rs` 的 11 项测试。
所以它不是「写了没人跑」的幽灵工具；这一轮的不变式是「生成不进门控，但生成逻辑被门控测试」。

接入 CI 与「发布目录」属后续变更（与 W41 落门控、W42 再接 CI 的次序一致）。

## 6. 为什么范围声明写进产物本身

物料清单最容易的误读是**当成安全结论**：它列了 299 个组件，看起来「清点过了就没问题」。
但它**不含**漏洞信息、不含签名、不含前端 npm 依赖、不含 ORT/CUDA/FFmpeg 等原生二进制、
不含模型与素材、也不含插件包自身的 BOM。

把这些写进 `metadata.properties`（`nai:sbom:scope` / `nai:sbom:excluded` / `nai:sbom:not-evidence-for`），
是为了让产物**离开本仓库后仍然自述其边界**——靠仓库内文档约束一份会外发的产物，等于没有约束。

## 7. 交叉核对：两处独立来源必须对得上

`cargo metadata` 与 `Cargo.lock` 是同一份解析结果的**两个独立视图**。生成时逐项核对：
锁文件里每一个带校验和的包，都必须在 metadata 里找到对应组件，否则**具名失败**，
而不是产出一份静默少一个包的物料清单。

本轮实测：299 组件 = 锁文件 299 个 `[[package]]`；281 个哈希**逐个**与锁文件比对，**0 处不一致**；
299 条许可**逐条**与 metadata 比对，**0 处改写或丢失**；`purl` 唯一。
