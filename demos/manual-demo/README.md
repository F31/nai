# Manual demo（人工辅助演示包）

代码/入口见 `docs/engine.md` 与 `docs/r1-delivery-plan.md`。这是顺序 3 的**仓内可验证部分**：
一套合法授权的演示素材与场景，加上一键预检；**不含安装器、Windows 设备输出或真实付费账户认证**。

## 内容

| 文件 | 作用 |
| --- | --- |
| `manual-session.json` | operator 场景（schema_1）：知识、prompt、budget、provider 预设、render 闭包 |
| `avatar/manifest.json` `config.json` | 2D 素材声明与图层配置（`software-rgba8-v1`） |
| `avatar/base.rgba` `closed.rgba` `open.rgba` | 三张真实 RGBA 素材，manifest 中 SHA-256 锁定 |
| `LICENSE` | 素材许可（`nai-demo-license`），manifest/scene 的 licenses 指向它 |
| `README.md` | 本说明 |

## 一键预检

```bash
NAI_DEMO_OPERATOR_KEY=... cargo run -p nai-engine -- \
  --data-root <dir> --workspace ws-primary --actor ops --profile prod \
  --preflight demos/manual-demo/manual-session.json
```

预检只读，不 provisioning、不起服务、不调用 Provider（放行可行时才发一次受预算约束的真实探针）。
每项检查输出 `passed` / `failed` / `not_ready` 与原因；任一 `not_ready`/`failed` 都拒绝整体放行
（沿 §12.2 与 nai-upgrade-core 预检的「未知与不足同等拒绝」），退出码非 0。预检结果写明
**未播放、未接设备**，不冒充 PlaybackAck。

## 真实行使

```bash
cargo run -p nai-engine -- --data-root <dir> --workspace ws-primary --actor ops --profile prod \
  --provision <runtime-profile.json> --operator-config demos/manual-demo/manual-session.json
```

`NAI_DEMO_OPERATOR_KEY` 指向的 Provider 端点对 demo 素材的准入、把关候选、审批与预览均需
`render` 配置；素材只有这里的三张占位图，属于**演练素材**，不构成真实播出形象。

## Agnes / SiliconFlow 参数场景

agnes-siliconflow-session.json包含用户选定的真实API地址、模型与音色，不含Key。也可在原生设置点击“填入 Agnes / SiliconFlow 参数（不含密钥）”。原人工演示知识仍标local_only，不能据此允许把知识外发；单项连通测试仅发送固定探针文本。每个供应商可显式测试一次，不发声，不自动转备，详见docs/provider-routing.md与2026-10-20-r1-provider-probe报告。
