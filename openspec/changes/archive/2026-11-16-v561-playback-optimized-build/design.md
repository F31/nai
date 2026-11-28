## 选择
沿用workspace已有按包优化策略，仅优化应用组合包。CPU转换原构建三组P50 20.5–21.4ms，候选0.193–0.210ms；Amdahl可消除上限约占40ms帧预算50%，但不能换算为全部句间延迟收益。无需改变图像算法、像素格式或新增组件。

## 边界
Rust内核和适配器依赖方向不变；控制协议与生成契约不变。聚合计时写本地JSON，内存O(1)，不经控制总线发送PCM或像素。window_submit含minifb节流，不是显示器呈现。音频start计时按spawn、pipe、OpenScript、renew及父等待分开，保留原认证和300ms租约。

## 验证
先pnpm exec openspec validate本变更，再普通cargo build核实优化参数；定向颜色、缓存、取消/时钟及clippy。Windows真实native-05先前显式等价编译参数实测通过，普通构建复用同一编译条件只授工程一致性，不追加模型调用。长期门沿prepared-narration-cache保留，独占OBS/安装包另验。

不改模型、默认画质或队列容量；不在准备前偷偷播放音频，也不从首声计时剔除设备启动。新依赖为0，现有debug信息与assert行为保留。
