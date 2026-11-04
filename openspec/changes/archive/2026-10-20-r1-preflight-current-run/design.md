# Design

## Context

固定 `operator-artifacts/preflight.json` 同时供宿主和连接后的控制面读取。上一轮报告不可充当本次进程完成证据。完整业务判定仍属于 engine。

## Decisions

- 宿主进程内使用互斥锁的 try_lock 包住失效、子进程运行、报告读取和失败清理；重叠请求返回 HOST_PREFLIGHT_BUSY，不排队。锁不涉及控制请求或静音。
- 启动前删除旧报告，NotFound 正常，其他删除失败拒绝启动。所有故障尽力清除本次产物，清理失败具名报告。该互斥仅覆盖单个宿主，不声称阻止外部 CLI 写入同一数据根。
- 最多读取 256KiB+1 字节，超界拒绝；必要布尔字段、非空 checks、唯一非空 name、封闭 status、字符串 reason 必须存在。宿主只校验结构及进程退出与 all_passed 一致性，不重新计算各轴或设备能力。
- 子进程原始输出可能包含凭据或诊断上下文，预检错误只返回稳定错误码；保留有界输出捕获供既有 provision 使用。
- 窗口开始一次预检时清空设置区与连接区的旧展示，避免宿主已失败而页面仍保留旧成功。
- 复用当前报告结构与错误码前缀，无 schema 变更、依赖或架构偏离，不新增 ADR。

## Verification

独立壳 cargo test/clippy/fmt；故障测试使用明确测试脚本，不用于认证。真实 engine 连续两次预检，读回同一报告路径且保持 connectivity=not_ready。根 full 门控和 OpenSpec 校验/归档。Linux 逻辑与 Windows 安装/签名/音轨认证分别记录。

## Limits

不实现设备播放、命名管道 ACL、凭据库或分发。进程崩溃后的历史报告不构成本轮成功；下一次运行会先删除它。外部 CLI 与宿主同时写同一数据根未建立跨进程锁。
