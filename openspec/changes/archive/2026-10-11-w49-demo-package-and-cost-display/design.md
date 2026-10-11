# 设计：O01 演示包就绪判定 + O05 升级成本显示

## 1. 范围与外部条件

| 项 | 库内可控 | 外部条件（本轮不做，如实登记） |
|---|---|---|
| O01 | §615 演示包的**构成与诚实性判定**；§612 两条时间目标的**记录判定** | 授权的演示形象内容、示例知识/场景正文、真实的一键连通、约 10 分钟与约 30 分钟的人工演练 |
| O05 | 成本**逐项准确列示**的呈现（未知 ≠ 0、部分和不作总额） | 真实升级界面的**渲染**、控制面暴露 plan/journal、生产接线 |

**登记纪律**：两者 `status` **均保持 `pending`**（O02 的 W38 先例：逻辑/界面出现不等于验收达标）。

## 2. O01：判定机制（`crates/nai-kernel/src/demo.rs`）

### 2.1 分层与依赖方向

放 `crates/nai-kernel`，与 `monitoring.rs`（O03）、`device.rs`（O04）同层同形。复用 `golden::Finding`。
kernel 现依赖 `nai-contracts` + `prost` + `serde_json`；本模块**只用 std**，依赖方向与包数不变（18 包）。

### 2.2 描述符

```text
DemoPackage {                      // §615 五要素 + §612 演练记录
  avatar: Option<DemoAvatar>,      //   授权的演示形象
  knowledge: Vec<KnowledgeSeed>,   //   示例知识
  scenario: Option<DemoScenario>,  //   示例场景
  provider_presets: Vec<ProviderPreset>, // Provider 预设
  connectivity: Option<ConnectivityTest>, // 一键连通测试
  cost: Option<CostDeclaration>,   //   成本预估
  drill: Option<DrillRecord>,      // §612 演练记录
}
```

- `DemoAvatar { profile_ref, license_ref: Option<String> }`——`license_ref` 是 §615「**授权的**」的落点。
- `ProviderPreset { id, credential_ref: Option<String>, embeds_secret: bool }`——凭据必须是**引用**
  （与 `schemas/config/provider.schema.json` 的 `credential_ref` 同纪律；S07 明文密钥）。
- `ConnectivityTest { probe: ProbeKind, claims_acceptance: bool }`，`ProbeKind = Live | Constant`。
- `CostDeclaration { lines: Vec<(&'static str, CostLine)> }`，`CostLine = Known(u64) | Unknown(&'static str)`。
- `DrillRecord { install_ready_at_ms, first_utterance_at_ms: Option<u64>, real_output_verified_at_ms: Option<u64> }`。

### 2.3 判定表（16 条稳定标记）

| 条件 | 标记 | subject |
|---|---|---|
| 声明了演示包但无形象 | `demo_avatar_missing` | `avatar` |
| 有形象但无授权引用 | `demo_avatar_without_license` | profile_ref |
| 示例知识为空 | `demo_knowledge_missing` | `knowledge` |
| 示例场景缺失 | `demo_scenario_missing` | `scenario` |
| Provider 预设为空 | `demo_provider_presets_missing` | `provider_presets` |
| 预设既无凭据引用又无密钥 | `provider_preset_without_credential` | preset id |
| 预设内嵌明文密钥 | `provider_preset_embeds_secret` | preset id |
| 一键连通测试缺失 | `demo_connectivity_missing` | `connectivity` |
| 连通测试是常量桩 | `connectivity_probe_is_stub` | `connectivity` |
| 冒烟结果被当验收达标 | `smoke_claims_acceptance` | `connectivity` |
| 成本预估缺失 | `demo_cost_estimate_missing` | `cost` |
| 未知分项未说明原因 | `cost_unknown_without_reason` | 分项名 |
| 演练记录缺项 | `drill_record_incomplete` | `drill` |
| 时间线不自洽（早于安装就绪） | `drill_timeline_inconsistent` | 里程碑名 |
| 首句耗时超过目标 | `first_utterance_beyond_target` | 实测毫秒 |
| 真实输出验证超过目标 | `real_output_beyond_target` | 实测毫秒 |

### 2.4 readiness（三态，`Defective` 优先）

```
audit 非空            ⇒ Defective
未声明（无任何要素）  ⇒ Blocked   （未开始，不是违规）
已声明但无演练记录    ⇒ Blocked   （§1261：未运行的验收记 Blocked）
否则                  ⇒ Ready
```

### 2.5 不判什么

不联网、不读文件、不测量耗时、不校验 JSON Schema（那是 `upgrade-core`/工具链的事）、
不新增 xtask 命令与证据目录（§30.1 未为演示包指定目录；真值来自真实演练）。详见 [ADR 0044](../../docs/adr/0044-demo-package-readiness-boundary.md)。

## 3. O05：成本显示（`apps/desktop/src/upgrade.ts`）

### 3.1 载荷形状

镜像 `nai-upgrade-core::cost` 的语义（未知 ≠ 0、部分和不作总额），以 JSON 表达：

```jsonc
{
  "currency": "CNY",
  "components": {
    "云资源": { "known": 1200 },        // 或 { "unknown": "<原因>" }
    "下载":   { "known": 0 },
    "存储":   { "unknown": "目标盘容量未探测" },
    "运行增量": { "known": 300 }
  }
}
```

### 3.2 呈现规则

- 逐项：`known` ⇒ `<分项>：<金额> <币种>`；`unknown` ⇒ `<分项>：未知（<原因>）`——**绝不**渲染成 0。
- 总额：全已知 ⇒ `合计 <金额> <币种>`；含未知 ⇒
  `下界 <floor> <币种>（<n> 项未知，非总额）`——**绝不**把下界说成合计。
- 严格解析：缺字段/多余字段/金额非安全的非负整数/原因为空 ⇒ 具名拒绝，不猜测。

### 3.3 为什么放 `apps/desktop/src/` 而不写进组件

同 W38 §30.1 的理由：可被机器检查的性质（未知不等于零、部分和不是总额）需要**脱离 DOM** 才能断言。
组件只把 `costRows()` 的结果贴回界面。

## 4. 验证

```bash
cargo xtask check-deps                                   # 18 包
cargo xtask check --registry                             # 注册表
cargo xtask check --profile portable                     # Rust 全量（含 demo 测试）
node --experimental-strip-types --test tests/ui/*.test.mjs  # UI 层（经 pnpm test:ui）
openspec validate --all --strict --archived
```

## 5. 环境边界

- 本机为 Windows MSVC 宿主（`source /c/Users/32791/.nai-msvc-env.sh`，`CARGO_TARGET_DIR=E:/nai-build-target`）。
- 前端逻辑层由 Node 22 直接加载（`--experimental-strip-types`），**不引入 DOM 库**，`tests/ui` 零新增依赖。
- 本轮为 **portable 全程可测**，无 `#[cfg(target_os = ...)]` 分支，无 CI workflow 改动。
