# R1 推进审视与操作者工作台交付

日期：2026-10-19；变更 `r1-operator-workbench`，关联 O02/O05。

## 审视与计划

用户指出工程治理占据迭代、实际产品闭环推进不足。核实 main.tsx、control.ts、upgrade.ts、pipeline.rs 与 registry 后采纳该方向：交付单位改为用户可完成的操作路径。

具体计划在 `docs/r1-delivery-plan.md`，当前优先完成手工事件→真实检索/API 生成→候选入库→桌面审批，接着推进 TTS/2D/设备输出、安装壳与演示包。H 系列跟随实际主链路接入，不再拆成七轮落日志任务。

需要纠正的判断：blocked 项可能同时缺仓内实现；implemented 项不全是只差设备；真实 Provider/审批/装配产生者的接线是工程工作，不能整体记成外部阻塞。上述核实不重做提交数/全仓行数统计，不把用户提供的规模数字冒充本轮重新测量。

发布线现测：B02/P01 是 r2，不属于 Slice-0/R1 的 47 条；O03 是 r1，仍是首版退出条件；A05 已是 tested，不能再归入 implemented 清单。

## 现在增加的用户能力

1. 审批列表实际显示 `display_text`；缺文本显示已有 FINAL_TEXT_GAP；保留候选哈希和二次确认。此前只有逻辑函数支持文本，React 行本身没有使用它。
2. 新增升级成本查询表单：输入包清单 JSON（512 KiB UI 上限），经真实 ControlClient.planUpgrade 请求既有控制面。显示四类成本、未知原因、下界与未验签说明；不提供假安装按钮。
3. 新增事务状态查询表单：输入显式 job id，经 fetchJob 查询既有端点；显示步骤、终态、清单、可选备份与错误码。未接安装器时如实呈现错误。
4. 新查询清除旧结果；断连/切换连接清除结果；复用既有 generation 机制忽略迟到响应；静音可以抢占查询。查询不自动重试。

成本计算、权限、HTTP schema、存储与内核均复用既有实现；没有新依赖、后台服务或验收编号，没有修改基线/registry 状态。

## 验证

- `pnpm exec openspec validate r1-operator-workbench --strict`：通过。
- `cargo xtask check --profile full`：通过（首次 240s 冷编译超时；依赖下载/缓存预热后第二次 600s 预算内完成，无修改 Rust）。包含 fmt、clippy、Rust workspace 测试和已有 Node/前端门控。
- full 通过后补了真实结果组件的静态渲染验证，最终再次运行 `node scripts/frontend-gate.mjs`：两包 typecheck/build、desktop 5 项、UI 78 项均通过。
- UI 新增 7 项：实际客户端计划查询、坏输入不发网络、503 不补零不重试、作业 id 查询、迟到成功/失败被丢弃，以及 React 组件的成本/回滚终态/文本转义静态渲染。
- 归档前 `openspec validate --all --strict`：44 项通过（43 主规格 + 当前变更）；`--all --strict --archived`：151 项通过。此前提交记载的 CI 失败不等于本机当前 CLI 必然失败；本次以上述实测为准，不声称远端 CI 已验证。
- 归档后两条校验分别为 43/43 主规格、152/152 归档通过；变更已归档到 `openspec/changes/archive/2026-10-19-r1-operator-workbench/`。full 是归档前执行的结果，最终前端检查已覆盖最后一次组件调整；未把它表述为归档后完整 profile 实测。

本轮请求测试使用明确的测试传输替身；React 测试使用既有 react-dom/server 生成静态 HTML。**没有运行浏览器交互、真实安装器或 Windows 设备验收，也没有调用付费 API。** 接线已进入实际页面不等于完整 O02/O05 达标，二者保持 pending。

## 使用

启动已有 engine 控制面和桌面开发入口 `pnpm --filter @nai/desktop dev`，输入真实本机地址与临时令牌后：

- 查看待审批列表中的最终文本；该步骤仍要求已有候选来源，页面不生成候选。
- 在「升级成本与事务查询」粘贴合规包清单并查询成本。
- 查询已有事务 id；成本查询不创建事务。安装器/作业面尚不可用时，查看具名错误而非虚构进展。

## 后续第一主线

下一项应完成真实候选生产与审批链路，交付证据是操作者输入一个问题后能在同一运行中的 engine/桌面看到并处理最终候选。仅增加 runtime 接口或函数测试不算完成该交付。
