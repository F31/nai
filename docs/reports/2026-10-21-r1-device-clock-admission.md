# r1-device-clock-admission（2026-10-21）

## 交付与边界

完成真实 WASAPI 设备时钟准入验证。新增 media-core::device_clock::DeviceClockCursor，按稳定频率转换原始 position 为帧，拒绝流身份不符、频率变化、位置倒退、超已提交帧和溢出；计数为帧而非交错声道值。新增 Windows 显式 ignored 无声设备测试，固定48k双声道，事件等待100ms、总预算3秒、缓冲最多4800帧，RAII停止流。

wasapi =0.23.0（MIT）只作为 Windows 开发依赖，Cargo.lock锁定新增9项依赖；无新增生产依赖或常驻资源，无 unsafe 项目代码。CPAL生产输出未替换，跨语言状态未修改，不需要契约再生成。设计见 [ADR0143](../adr/0143-device-clock-admission.md)。

## 实测

- WSL `cargo test -p nai-media-core --lib device_clock --offline`：3 passed，覆盖换算、频率/身份/倒退/未提交、零值/溢出。
- Windows `cargo test -p nai-output-keeper --test device_clock --offline -- --ignored --nocapture`：1 passed，1.92秒；频率384000、49次观测、实际24170帧；停止位置203520，等待100ms后保持不变。仅提交零样本。日志 `.nai-diagnostics/device-clock-native.log`。
- media-core lib clippy -D warnings、workspace fmt --check通过；Windows新增设备测试 clippy -D warnings通过。

没有全仓测试、安装替换、用户数据库/凭据改动或付费API请求。此测试观察实际 IAudioClock position，不使用QPC或墙钟推算已播帧。它验证原生接口和单位，不证明扬声器声音、完整A/V播放、候选租约链、OBS录制或产品认证；registry状态保持不变。

## 下一步

将安全 WASAPI 接口接入常驻输出线程（每会话一条stream），按实际成功提交的绝对帧范围绑定每cue，撤权/epoch/期限/error任一不符均不补完成证据。补齐状态契约、宿主独立复检与真实设备取消/换代测试后，再决定完成ACK和检查点写入。自动讲品与插问恢复、真人麦克风、OBS整场验收仍未完成。

收尾：验收注册表57/57通过；主规格严格校验44/44通过，变更已归档为2026-10-21-r1-device-clock-admission。
归档规格校验186/186通过。
