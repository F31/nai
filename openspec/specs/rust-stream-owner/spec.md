# rust-stream-owner Specification

## Purpose
定义Rust对真实增量worker的独占装配、私有启动、维护预热及实际PCM前沿和共享像素复检，要求取消有界清退且不回退旧槽身份；供应商完成证据必须贯通到同源渲染，设备播放与低延迟验收仍独立，不以内部结果替代直播通过。

## Requirements

### Requirement: Rust独占增量worker装配
Rust owner SHALL 私有启动并独立验证真实Ready、签名任务事件和共享像素；所有通道有界，维护期限不放宽业务期限。

#### Scenario: 帧超过实际已交付源
- **WHEN** worker帧源起点超出实际已交付PCM或EOS后帧总量不符
- **THEN** 拒绝结果并回收worker，不以声明上限替代真实输入。

### Requirement: 取消清退不复活旧槽
Rust owner SHALL 在取消父操作后以独立有界清理读取终止事件，仅认证渲染取消且完整raw通道仍有效时才释放在途槽。

#### Scenario: 取消后新任务
- **WHEN** 取消已被GPU确认且原始通道完整
- **THEN** 更高任务与epoch可以复用模型，payload ID不回退，旧帧不进入新任务。

### Requirement: 供应商完成证据贯通Rust渲染
Rust composition SHALL 使用同一Operation将本地CosyVoice规范PCM交给增量owner，只有适配器完成回执验证且源样本/摘要一致后才提交worker EOS；失败取消临时任务，不把已有帧作为成功结果。

#### Scenario: 供应商生成失败
- **WHEN** 收到部分PCM后网络或应用完成回执失败
- **THEN** 取消当前GPU任务，不签发EOS，不重试或复用部分音频。
