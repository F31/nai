## Decisions
V4.3作为后续重构目标，不保留旧协议/ABI/表单/场景JSON兼容层；旧数据与许可仍受保护，当前安装不替换。产品中心为LiveShow/LiveSeries，默认Assisted与私有候选；DeliveryUnit与媒体子状态分离。保留可证明的底层算法/测试资产，不以旧测试数量换新ACC通过。单存储owner、按需进程和公共/私有sink分离；采用现有schemas/作为唯一契约源，不重复建设protocols/源目录。
## Verification
严格校验规划OpenSpec；核对全部ACC/INV覆盖、任务依赖无环、文档引用及入口一致性。文档检查不代替Rust/Windows/平台/付费/商业验证。本轮无运行变更，不跑全仓测试。
