## Why
本机音频只能使用默认声卡，无法选择耳机/虚拟输出，也没有绑定实际设备的可见信息。这使私有监听难以规划，设备变化时操作者也无法核对实际声音去向，需要明确的设备选择、显示及失效停止边界。
## What Changes
- 显式准备本次输出，独立worker枚举并保留最多16个实际设备对象，以本次输出session和短device ID引用。
- 默认按钮选择系统默认设备；可手动打开设备选择，选择后才打开并播放。选定设备失效拒绝，不自动回退。
- 输出状态显示实际名称，仍为私有监听，无公共或平台许可。
## Capabilities
### New Capabilities
- `output-device-binding`: 单次原生输出的设备对象绑定与可见选择。
### Modified Capabilities
## Impact
源Protobuf、worker/根/宿主和试听UI；复用短租约/P0/业务source守门，无付费调用、新常驻服务或持久设备身份声明。
