# W32 实测报告：写实准入 Gate 判定骨架（B02 机制侧前置解耦）

- 日期：2026-10-10
- 变更：`openspec/changes/w32-realistic-gate-verdict/`
- 决策：[ADR 0034](../adr/0034-realistic-gate-verdict-boundary.md)；指南：[docs/realistic-gate.md](../realistic-gate.md)
- 判据：`tests/registry.yaml` B02（回指 D12）

## 结论

B02 与 D12 的**真值**（合法完整模型 + Windows/CUDA 目标机 + 与直播工具并发实测）需要外部条件，本轮不可达，两项**保持 `blocked`**。
本轮做的是 **B02 的判定骨架**：§8.2 的表只有四行，每行一门，判据此前没有落点——于是「写实 Gate」在执行层等于一句**无从判定**的要求。本轮把
§8.2（并 §8.1 档位、§8.4 回退、§26.5 执行后端、§12.5 帧率与音画偏差目标、§1311 纪律）的判定口径落进
`crates/nai-kernel/src/realistic.rs`，四处最容易含糊的地方被钉死：

1. **四门是合取、各自独立**——G0/G1/G2/G3 各自 `Passed/Failed/Unmeasured`，总体 `Acceptance` 由四门**派生**，**没有任何单分数**；
   「缺测」既不是通过也不是零。
2. **「全 GPU」是放置记录的性质，不是运行回执**——`cuda_ep_registered` 只是审计线索（§26.5 原文点名禁止由它宣称全 GPU）。
3. **「自动回退 2D 不等于写实验收通过」**——回退是 §8.4 的**运营记录**，与准入门控**正交**；只有「以回退构成验收」的**声明**才违规。
4. **§1311 纪律**——缺模型 ⇒ `Blocked`；NotApplicable 只用于**不要求写实**的档位（Lite）；**无豁免路径**。

**本轮不产生任何真实证据**：真实权重与授权文件、Rust/ORT 推理、目标机实测均来自外部，不在此仓库生成。**不新增 xtask 命令与证据目录**：
§30.1 只为 RAG 黄金集指定了 `tests/rag-golden`，为写实 Gate 臆造证据目录属过度绑定；G2 真值来自目标机实测而非仓库载荷。

## 目标行为

| 判据（出处） | 实现 | 关键设计 |
| --- | --- | --- |
| 四门独立（§8.2） | `Gate::ALL` / `GateReport::verdict` | 四门各自三态，总体由 `acceptance()` 派生 |
| 无单分数 | 不存在任何合成方法 | `Acceptance::{Accepted,NotAccepted,Unmeasured,Blocked,NotApplicable}` |
| 缺测非过（§8.2/§12.5） | 任一门 `Unmeasured` 即阻断 `Accepted` | 帧率无窗口 / 偏差样本不足 / 温度未读 ⇒ `Unmeasured` |
| G0 五类权利（§8.2） | `RightKind::ALL`（权重/代码/训练素材/肖像/声音） | 依据空白不算成立（「许可引用存在 ≠ 证据通过」） |
| G0 完整链路（§8.2） | `pipeline_obtainable` + `undeclared_components` | 缺组件留未声明 Python 服务 ⇒ `Failed` |
| G1 四段对齐（§8.2） | `Stage::ALL` / `StageAlignment` | 任一段 `Diverged` ⇒ `Failed`；`NotRun` ⇒ `Unmeasured` |
| G1 精度有序（§26.5） | `Precision` + 逐条拒绝 | 非 FP32 先于 FP32 认证 ⇒ 拒；INT8 无校准 ⇒ 拒 |
| G2 目标机 + 并发（§8.2） | `target_machine` / `concurrent_with_live_tool` | 空白 ⇒ `Unmeasured`；非并发 ⇒ `Failed` |
| G2 帧率稳定（§12.5） | `FrameRate` 窗口**下限** | 稳定 25fps；否决会被突发拉高的平均 |
| G2 音画偏差（§12.5） | 复用 `LatencySamples::percentile(P95)` | ≤80ms；<20 样本未测量；恰好 80ms 判达标 |
| G2 全 GPU（§26.5） | `PlacementReport::is_full_gpu()` | 非空且全 GPU；注册不是放置；虚假声明 ⇒ `Failed` |
| G3 七场景（§8.2） | `StabilityScenario::ALL` | 失败 ⇒ `Failed`；未运行 ⇒ `Unmeasured`；通过无证据 ⇒ `Failed` |
| 回退正交（§8.2/§8.4） | `FallbackRecord` 不进 `verdict`/`acceptance` | 只有「以回退构成验收」违规 |
| 缺模型（§1311） | `model_present = false` ⇒ `Blocked` | 缺模型却有门判通过被审计报出 |
| 不适用（§1311） | `Profile::requires_realistic()` | 仅 Lite 成立；Standard/Pro 声明不适用被审计报出 |
| 审计纯函数（B02） | `audit_gate` | 十四条稳定标记，逐条可构造 |

## 测试

`cargo test -p nai-kernel --test realistic`：**37 passed / 0 failed**。

| 组 | 覆盖 |
| --- | --- |
| 四门与派生（8） | 四门标签稳定；全测报 `Accepted` 且审计为空；一门缺测 ⇒ `Unmeasured`；一门失败 ⇒ `NotAccepted`；缺模型 ⇒ `Blocked`；缺模型却有门通过被审计报出；Lite 未启用 ⇒ `NotApplicable`；Standard 声明不适用被审计报出 |
| G0（5） | 五类权利齐全通过；缺肖像 ⇒ `Failed` 并指名；空白依据不成立且被审计报出；留组件在未声明 Python 服务 ⇒ `Failed`；链路不可取得 ⇒ `Failed` |
| G1（6） | 四段对齐通过；生成段发散 ⇒ `Failed`；后处理未运行 ⇒ `Unmeasured`；固定输入空白 ⇒ `Unmeasured`；仅认证 FP16 ⇒ `Failed`；INT8 无校准 ⇒ `Failed`；INT8 有 FP32 + 校准 ⇒ `Passed` |
| G2（8） | 目标机空白 ⇒ `Unmeasured`；未并发 ⇒ `Failed`；24fps ⇒ `Failed` 且 25fps ⇒ `Passed`；窗口下限而非平均；81ms ⇒ `Failed` 且 80ms ⇒ `Passed`；样本不足 ⇒ `Unmeasured`；缺 CPU/显存 ⇒ `Unmeasured`；`cuda_ep_registered` 无放置却宣称全 GPU ⇒ `Failed` 且两条审计；有 CPU 回落却宣称全 GPU ⇒ `Failed`；如实记录 CPU 回落且达标 ⇒ `Passed` |
| G3（4） | 七场景齐全通过；OOM 失败 ⇒ `Failed`；形象切换未运行 ⇒ `Unmeasured`；声明通过无证据 ⇒ `Failed` 且被审计 |
| 回退（4） | 回退不救门（`Failed` 仍 `NotAccepted`）；声明以回退构成验收被审计且不改准入；回退缺授权/不在句间/未记日志三条分别构造；合规回退单独无 finding |

## 实跑

```
$ cargo xtask check --profile portable
... 62 test binaries + 17 doc-test groups, 558 passed / 0 failed
portable: engineering foundation checks passed; device/SDK/GPU/install certification not performed
EXIT=0

$ cargo xtask check-deps
dependency boundaries passed: 18 workspace packages (normal/build/dev/target declarations)

$ cargo xtask check --registry
admission registry check passed (V3.6 acceptance + D01-D14 mapping)
```

门控目标数由 W31 的 78 升至 **79**（62 Running + 17 Doc-tests）：新增一个集成测试目标 `tests/realistic.rs`。**首次门控遭遇宿主 LNK1104**
（`nai_engine` / `nai_budget_core` / `accounting` / `nai_control_api` 逐包被锁），按既有处置「按报错路径定向清理被锁 exe + 重试预链」
**2 轮收敛**，随后一次通过——宿主对新生成 exe 的瞬时锁，与代码无关。

## 依赖边界

- 落 `nai-kernel`（只依赖 contracts），复用 `metrics::LatencySamples` 与 `golden::Finding`（后者 `Finding::new` 由 `fn` 提升为 `pub`）。
- `check-deps` 仍 **18 包**、方向不变；**外部依赖零新增**（仅 `nai_contracts::v1::ErrorCode` 与 `std`）。
- 不新增 crate；`Cargo.lock` 不变。

## 未覆盖边界

- **不含任何真实证据**：真实权重与授权文件、Rust/ORT 推理、Windows/CUDA 目标机实测、与直播工具并发测量均由外部提供。测试中的证据是**构造的**，只证明口径与派生可执行。
- **不接 ORT/CUDA/直播工具**：不加载模型、不推理、不读 GPU。
- **不改 `model-package-integrity`**（D12 前置）：完整性 ≠ 认证；本模块消费的是「完整性通过之后」的 G0–G3 证据。
- **不加载体**：待真实模型轨立项并确定证据落点后再补 xtask 命令，**无需改本模块判定**。
- 注册表 **B02 与 D12 保持 `blocked`**：判定机制（W32）在 notes 中保留，证据齐全仍待外部投入。
