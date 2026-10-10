# realistic-gate Specification

## Purpose
把 §8.2「写实模型准入 Gate G0–G3」的**判定口径**落成在 portable 下可执行、可测试的骨架：四门各自独立判定、缺测不得被读成通过、2D 回退不得冒充写实验收、无合法完整模型一律 Blocked。本能力只判定**证据是否满足门控**，不产生任何真实权重、推理或目标机测量；这些真值由外部条件提供，未提供时认证保持 Blocked。

## Requirements

### Requirement: Four independent gate verdicts
写实准入 SHALL 由四个**互相独立**的门（G0 来源与授权、G1 正确性、G2 实时性、G3 稳定性）各自判定，每门的结果 MUST 为 `Passed`、`Failed` 或 `Unmeasured` 三者之一。总体准入 MUST 由四门结果派生，MUST NOT 把任一门的结果折进另一门，也 MUST NOT 由任何单一分数替代四门。

#### Scenario: One gate unmeasured blocks acceptance
- **WHEN** G0 与 G3 判定为 `Passed`，G1 为 `Passed`，而 G2 因样本不足判为 `Unmeasured`
- **THEN** 总体准入为 `Unmeasured`，且逐门结果仍各自可见

#### Scenario: Measured requires the measurement
- **WHEN** 任一门的判定为 `Unmeasured`
- **THEN** 总体准入 MUST NOT 为 `Accepted`

### Requirement: Source and rights completeness
G0 SHALL 逐项要求**权重、代码、训练素材、肖像、声音**五类使用权利成立，且完整链路可取得。任一权利缺失、完整链路不可取得、或存在被留在未声明 Python 服务中的缺失组件，MUST 判为 `Failed`。

#### Scenario: A missing right fails the gate
- **WHEN** 肖像与声音权利未成立
- **THEN** G0 为 `Failed`，并逐项报出缺失的权利

#### Scenario: An undeclared component fails the gate
- **WHEN** 完整链路中有组件仍留在未声明的 Python 服务里
- **THEN** G0 为 `Failed`，不得以「可取得」的名义放行

### Requirement: A rights reference is not the evidence
G0 SHALL 要求每类权利的**依据**非空；仅记录一个空洞引用（空白依据）MUST NOT 被视为权利已成立。存在空白依据的权利 MUST 使 G0 为 `Unmeasured`，并被审计报出。

#### Scenario: Blank basis is not established
- **WHEN** 某类权利标为已成立但依据为空白
- **THEN** 该类权利 MUST NOT 计为已成立，G0 不判为 `Passed`，审计报出该项

### Requirement: Correctness alignment across the whole pipeline
G1 SHALL 在**固定输入**下逐段对齐预处理、音频特征、生成、后处理四段。任一段发散 MUST 判为 `Failed`；任一段未运行 MUST 判为 `Unmeasured`；固定输入未指明 MUST NOT 判为 `Passed`。缺组件被留在未声明 Python 服务中 MUST 判为 `Failed`。

#### Scenario: A diverged stage fails
- **WHEN** 生成段在固定输入下与原实现发散
- **THEN** G1 为 `Failed`

#### Scenario: A stage not run is unmeasured
- **WHEN** 后处理段尚未运行
- **THEN** G1 为 `Unmeasured`

### Requirement: Precision certification order and quantized variants
G1 SHALL 遵守 §26.5：先用 FP32 与原流程逐段对齐，再认证 FP16；认证 FP16 而未先通过 FP32 MUST 判为 `Failed`。认证 INT8 而无量化与校准报告 MUST 判为 `Failed`。

#### Scenario: FP16 certified without FP32 baseline
- **WHEN** 只认证了 FP16，FP32 尚未对齐
- **THEN** G1 为 `Failed`

#### Scenario: INT8 without calibration
- **WHEN** 认证 INT8 但未提供量化与校准报告
- **THEN** G1 为 `Failed`

### Requirement: Realtime measured on the target machine under load
G2 SHALL 要求测量发生在**目标机**上且**与直播工具同时运行**。非并发测量 MUST 判为 `Failed`；目标机未指明 MUST 判为 `Unmeasured`。帧率 MUST 判定为是否稳定达到 25fps，音画偏差 MUST 以 P95 判定是否 ≤80ms；帧率或分位未测得 MUST 判为 `Unmeasured`，分位样本不足 MUST NOT 产生一个数值。显存、CPU、温度三项任一未测得 MUST 使 G2 不为 `Passed`。

#### Scenario: Realtime measured off the target machine
- **WHEN** 帧率与偏差在非目标机上测得
- **THEN** G2 为 `Unmeasured`，不得据以判定实时性

#### Scenario: Measured without the live tool
- **WHEN** 测量未与直播工具同时运行
- **THEN** G2 为 `Failed`

#### Scenario: Below the frame target
- **WHEN** 实测帧率低于 25fps，或音画偏差异步 P95 超过 80ms
- **THEN** G2 为 `Failed`

#### Scenario: Partial realtime sampling
- **WHEN** 帧率与偏差达标，但温度未测得
- **THEN** G2 为 `Unmeasured`，不得判为 `Passed`

### Requirement: Placement is recorded, not inferred from EP registration
G2 的证据 SHALL 记录实际的节点放置与 CPU 回落。CUDA EP 注册成功 MUST NOT 被视为全 GPU 的证据。宣称全 GPU 而放置记录为空、或存在 CPU 回落时，MUST 判为 `Failed`，并由审计报出。

#### Scenario: CUDA EP registered is not placement
- **WHEN** 报告仅记录 CUDA EP 注册成功而无任何节点放置记录
- **THEN** 不得宣称全 GPU，G2 为 `Failed`

#### Scenario: Full GPU claimed with CPU fallbacks
- **WHEN** 报告宣称全 GPU 但放置记录中存在 CPU 回落
- **THEN** G2 为 `Failed`

### Requirement: Stability across the seven scenarios
G3 SHALL 覆盖静音、取消、句边界、长播、OOM、进程重启、形象切换与输出恢复七个场景。任一场景失败 MUST 判为 `Failed`；任一场景未运行 MUST 判为 `Unmeasured`；声明已通过而无证据引用 MUST 判为 `Failed`。

#### Scenario: OOM scenario failed
- **WHEN** OOM 场景未按预期静音并恢复
- **THEN** G3 为 `Failed`

#### Scenario: A scenario not run
- **WHEN** 形象切换与输出恢复场景尚未运行
- **THEN** G3 为 `Unmeasured`

#### Scenario: Passed claimed without evidence
- **WHEN** 某场景标为已通过但未给出证据引用
- **THEN** G3 为 `Failed`

### Requirement: Falling back to 2D is not realistic acceptance
一次 2D 回退 MUST NOT 改变任何门的判定，也 MUST NOT 使总体准入变为 `Accepted`。声明「以 2D 回退达成写实验收」MUST 被判为违规并由审计报出。回退记录 MUST 具备运营预先授权、在句间发生且已写日志，缺任一项 MUST 被审计报出。

#### Scenario: Fallback does not grant acceptance
- **WHEN** 四门中存在 `Failed`，并发生一次 2D 回退
- **THEN** 总体准入仍为 `NotAccepted`

#### Scenario: Fallback claimed as acceptance
- **WHEN** 报告声明以 2D 回退构成写实验收
- **THEN** 审计报出违规，且总体准入 MUST NOT 为 `Accepted`

#### Scenario: Fallback without authorization
- **WHEN** 发生 2D 回退但未经运营预先授权，或不在句间，或未写日志
- **THEN** 审计逐项报出

### Requirement: Missing model blocks and no gate may be waived
没有合法完整模型时，写实认证 MUST 为 `Blocked`；已有任一门的 `Passed` 结果在模型缺失时 MUST 被审计报出。任何门 MUST NOT 通过豁免转为通过。

#### Scenario: No model present
- **WHEN** 报告标记模型不完整
- **THEN** 总体准入为 `Blocked`，不因任何已给证据而变成 `Accepted`

### Requirement: Not applicable only for profiles that do not require realistic
「不适用」MUST 只用于本档位确实未启用写实的情形。档位要求写实（Standard/Pro）而声明「不适用」MUST 被审计报出，且总体准入 MUST NOT 为 `NotApplicable`。

#### Scenario: Lite profile does not require realistic
- **WHEN** 档位为 Lite 且声明未启用写实
- **THEN** 总体准入为 `NotApplicable`

#### Scenario: Standard profile claims not applicable
- **WHEN** 档位为 Standard 而声明写实不适用
- **THEN** 审计报出该项，总体准入 MUST NOT 为 `NotApplicable`

### Requirement: Gate audit is a pure function of the report
审计 SHALL 是对报告自身字段的**纯函数**，报出全部命中项，使每条规则都能被直接构造与验证，MUST NOT 被判定函数的保证架空。

#### Scenario: Every finding is constructible
- **WHEN** 直接构造一份含某违规的报告
- **THEN** 审计报出对应的稳定标记，无需经由判定函数
