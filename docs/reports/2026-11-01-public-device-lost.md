# 公共设备流失败停播

## 结果

同一条受控 WASAPI 流的探测端一旦报告失败，公共执行停播。理由是 `DEVICE_LOST`。设备时钟观测清空，当前帧清空，选帧器撤销，执行栅栏停止。同一轮循环不再用失联前的观测选帧。

宿主接纳规则把 `DEVICE_LOST` 列为已停播理由。带着旧 `device_clock` 的失联状态被拒绝。未停播却自称 `DEVICE_LOST` 也被拒绝。

`cargo test -p nai-media-core --lib public_device_loss --offline` 1 项通过。`cargo test -p nai-workbench-host --lib device_clock --offline` 3 项通过。`cargo clippy -p nai-media-core -p nai-workbench-host -p nai-public-output-keeper --offline -- -D warnings` 通过。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。1.3.10.2 已按这次接线勾选。1.3.10 整项和 1.3.11 没有勾选。

## 没有改变的路径

没有拔真实声卡，没有跑 keeper 的真实设备套件，没有公共音画或 OBS 实录。没有把 MuseTalk、960×540 研究帧或 Python 路径接入公共窗口。没有改协议字段，没有授予播放许可。164 项验收仍全部 pending。

随后静音设备出画记在 [静音设备上按已播放样本出画](2026-11-01-public-played-av.md)。
