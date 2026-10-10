## 1. 规格与设计

- [x] 1.1 建立 OpenSpec 变更 `w39-ipc-untrusted-input`，新增 `plugin-hosts-and-ipc` 差量需求（3 条需求 / 6 个场景），`validate --strict` 通过。
- [x] 1.2 落设计决策 D1–D6（确定性种子、断言全域性/有界性而非"找到漏洞"、非空洞性守卫、四入口独立、复用既有装配、实现预期零改动）。
- [x] 1.3 把审计结论写成发现 F1–F3（三个解码入口中两个零测试、路径面仅 1 项手写用例、顺序不变式未被钉住），各附可核对文件行号。

## 2. 随机输入 harness

- [x] 2.1 新增 `crates/nai-plugin-hosts/tests/untrusted_input.rs`：自带 splitmix64 固定种子 PRNG（零新增依赖）。
- [x] 2.2 raw 帧流：随机字节/随机长度前缀（含 `0xFFFFFFFF`）喂 `frame::read`，断言只产生 `Ok(Some)`/`Ok(None)`/具名错误，不 panic。
- [x] 2.3 有界性：长度前缀 > `MAX_FRAME_BYTES` ⇒ `RESOURCE_EXHAUSTED`，且不按声明长度分配。
- [x] 2.4 往返：随机 body（≤64KiB）经 `encode` → `read` 必须逐字节相同。
- [x] 2.5 结构化 envelope：合法握手 + `Envelope`（协议版本合法、`message_type`/payload 任意）驱动 `serve`，断言具名错误且不 panic。
- [x] 2.6 顺序不变式：取消置位 / 期限已过时，**任意**输入都返回 `CANCELLED` / `DEADLINE_EXCEEDED`。
- [x] 2.7 路径：由 `{正常段, "..", ".", 绝对, 空段, 长段}` 随机组合的路径串喂 `verify_within_root`，含 `..` 必拒、通过者必在 canonical root 之下。

## 3. 非空洞性守卫

- [x] 3.1 每类断言维护命中计数，末尾断言计数 ≥ 阈值（每类形状/错误码至少一次）。
- [x] 3.2 计数不足即测试失败（防止生成器只产合法输入把测试变成橡皮图章）。
- [x] 3.3 反向自检：临时把某一类的阈值调高到不可能命中，确认测试**确实会失败**，然后恢复。

## 4. 实现与实测

- [x] 4.1 `cargo test -p nai-plugin-hosts --offline` 通过；记录新增测试项数与新增测试目标数。
- [x] 4.2 若 harness 发现真实缺陷：修实现并在报告如实记录缺陷、根因与修复；若未发现：报告中写「**未发现**」而**不**写「已验证安全」。
- [x] 4.3 断言运行时长有界（记录实测耗时），不成为门控瓶颈。

## 5. 文档、registry 与闭环

- [x] 5.1 `docs/hosts.md` 新增一节：攻击者可控输入面、测试口径（随机输入测试 ≠ 覆盖率导向模糊测试）与运行方式。
- [x] 5.2 新增报告 `docs/reports/2026-10-11-w39-ipc-untrusted-input.md`。
- [x] 5.3 `tests/registry.yaml` S11 notes：记录本轮证据、**明确边界**（非覆盖率导向、无 libFuzzer、未测真实命名管道对端身份）；状态保持 `tested` 且此时有据可依。
- [x] 5.4 `docs/development-plan.md` 与 `AGENTS.md` 记录本轮。
- [x] 5.5 顺序纪律：`check --secrets` → `check --registry` → `check-deps` → 前端 typecheck/build/desktop 测试 → `test:ui` → portable 全量门控 → 勾 tasks → `openspec archive -y` → `validate --all --strict --archived` 全通过。
- [x] 5.6 一次 commit + push；commit 前确认 `.env` 未被跟踪、无凭据入库。

## 里程碑切分

- **M1（§1–§3）**：规格、设计与非空洞性 harness 到位，`cargo test -p nai-plugin-hosts` 通过。
- **M2（§4–§5）**：实测数字入报告，文档与 registry 更新，门控与归档闭环。
