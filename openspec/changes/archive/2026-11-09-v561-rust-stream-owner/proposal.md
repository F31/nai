# Proposal

## Why
真实worker2已用Python诊断父进程验证，尚非Rust引擎装配。需要Rust负责私有启动、独立维护期限、帧复检/释放和取消后清退，才能接业务TTS及设备时钟。

## What Changes
新增Rust增量Resident，复用共享启动环境与已有只读帧Ingress/raw组件。签名验证Ready/任务/序号、未知总长按实际已交付PCM限制帧、独立EOS与渲染完成、取消清退原始槽且不回退ID。原生Rust→真实GPU→共享像素及恢复验证。

## Impact
当前Rust主控组件和明确诊断入口，旧GUI和安装版不自动切换；本切片不授真实TTS或设备播放完成。
