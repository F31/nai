# 隔离实验单次短期租约与失联到期撤销

## 结果

实验设备栅栏只签发一次本地短期租约，最长 500ms，不续租。到期视为失联并撤销：已到期的写入不再提交，同一栅栏不能再激活或延长。外部受控武装状态不在本地到期。栅栏状态改为具名常量，`allowed`/`revoked` 先拍一次原子快照。零 TTL、超过 500ms、到期后禁止重激活仍由既有单测覆盖。

`cargo test -p nai-audio-cpal --lib --features clocked-experiment --offline -- --test-threads=1` 22 项通过，含新增的 `expired_local_lease_revokes_as_loss_of_contact`。`cargo clippy -p nai-audio-cpal --features clocked-experiment --all-targets --offline -- -D warnings` 通过。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。1.3.15 已按这次验收勾选。1.3.10 整项没有勾选。

## 没有改变的路径

没有 OBS 实录，没有口型评分，没有平台开播。没有可信跨进程续租、独立故障隔离或生产许可。没有替换 `open_controlled`。没有授予播放许可。Drop 仍可能无界 join。没有重跑静音设备硬件项。164 项验收仍全部 pending。
