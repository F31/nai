# W06 2D 渲染核心实测报告

日期：2026-10-09。基线：V3.6 §26.3/26.4/26.7/31.2，验收来源 tests/registry.yaml（A01）；OpenSpec w06-avatar-2d-core。
环境：Linux/WSL，Rust1.96.0、Node22.23.1、pnpm8.15.0、OpenSpec1.14.1。没有调用付费 Provider、GPU、SDK或Python。

## 实现

- 新增 plugin 层 `nai-avatar-2d`，复用已锁定 contracts/plugin-sdk/media-core/serde/jsonschema/sha2/prost；没有新增外部依赖或常驻进程。
- 先增加 avatar-2d JSON Schema、生成 TS 契约，再实现消费端；保留通用 AvatarManifest 和所有 Protobuf 消息不变。
- 精确三层 raw RGBA8、许可 grants、SHA256、尺寸/位置/路径声明/闭包检查；只接受 root 安全读取的不可变bytes，不从清单路径打开文件，不标 Ready。
- 有界 mono f32 PCM（最多96000样本）、seq/样本连续性、finite幅值、session/epoch/utterance和EOS校验。
- 样本窗口 RMS、噪声门限与攻击/释放指数平滑；非整数fps按绝对有理样本边界映射。逐帧输出实际RGBA像素与FrameMetadata，EOS尾帧的duration保留真实尾样本数。
- 每次输入/渲染复检授权/context；逐行/逐块与提交前检查，失效流不可恢复旧授权，失败不消费PCM/推进时轴。根调用方仍须由keeper门控输出。
- 每次至多一帧，无内部帧队列；像素为owned二进制，不经JSON/Base64，不声称零拷贝或硬件ACK。

## 实测

| 命令 | 结果 |
|---|---|
| `cargo test -p nai-avatar-2d --offline` | 9项集成测试通过 |
| `cargo xtask check-deps` | 14个workspace包依赖边界通过 |
| `cargo xtask check --profile portable` | registry/fmt/clippy全目标-D warnings和271项workspace Rust测试通过 |
| `cargo xtask schema --check` | 无漂移；27 capabilities、109 protobuf messages |
| `cargo xtask test --suite contracts` | 8项Rust契约/11项schema/API测试、Rust/TS/C++编译及12类消息双向互通通过 |
| `pnpm typecheck`、`pnpm build` | contracts-ts和desktop通过 |
| `cargo xtask check --registry` | 最终A01状态/证据引用检查通过 |
| `OPENSPEC_TELEMETRY=0 pnpm exec openspec validate w06-avatar-2d-core --strict --no-interactive` | 通过 |

测试使用极小的自建fixture像素，核对alpha参考值、静音/门限/攻击/释放、PCM正负极性、不同时轴分片下相同像素、30000/1001fps采样边界及EOS尾样本。拒绝测试覆盖篡改/缺失/额外/重复/未授权素材、错误位置/尺寸/fps/模型/参数、PCM格式/序号/缺口/NaN/幅值/满额、错误epoch/取消/许可撤销/期限以及超大cue。

初次完整检查暴露生成脚本将avatar-2d导出为非法TS namespace的问题（仅处理连字符后字母）。已修复源生成器的数字后缀转换并重新生成；之后完整contracts与前端检查通过，未手改生成代码。CMake出现约0.13秒文件时钟偏差警告，实际C++编译/链接与跨语言执行全部成功。

## 状态与后续

- A01保持implemented：新增真实像素生产核心证据，不标无独显桌面演练/设备/降级视图完整tested。
- W06部分实现；独立render-worker、protected共享内存句柄/槽位、keeper帧消费与fallback、宿主capability注册及生产root接线尚未实现（未开始）。
- Windows窗口/设备/桌面帧率认证未执行（当前WSL环境Blocked）；生产形象包/真实许可审批/整机资源预算没有证据，不能以测试fixture替代。
- 逻辑载荷边界：三层约24MiB、当前帧8MiB、有效PCM约375KiB；分配器/容器capacity/root输入与调用方持有帧另计，实际RSS及1080p性能没有测量。
- 下一主线：W05/W06共享内存数据面与独立worker/root装配，再做真实进程故障及keeper待机切换验证。

## 归档

已归档至 `openspec/changes/archive/2026-10-09-w06-avatar-2d-core/`，6/6任务完成；新增主规格avatar-2d-rendering（3条需求）。
`OPENSPEC_TELEMETRY=0 pnpm spec:validate`：22个主规格全部通过；
`OPENSPEC_TELEMETRY=0 pnpm exec openspec validate --archived --no-interactive`：20项归档全部通过。
