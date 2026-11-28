# Proposal

## Why
连续GPU会话和原始通道分别验证过，但生产worker6仍只接完整文件。需要真实常驻增量worker入口，将签名控制、共享帧、raw和GPU连接并区分输入EOS与渲染完成，避免将组件成功当作整链成功。

## What Changes
增量协议升为2，增加真实Ready、Frame、RenderComplete/RenderCancelled和Release；生成四语言。新增真实worker入口，加载/编译预热后才Ready，同一模型支持顺序新任务，控制与GPU线程分开。不得自动切换GUI或将独立诊断当设备播放。

## Impact
新协议及worker装配、定向故障与原生GPU诊断。旧增量协议1拒绝，无兼容层；完整源诊断worker6单独保留现有能力。
