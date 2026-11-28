# V5.6.1 卸载与重装

日期：2026-11-04。任务：1.7。状态：脚本已实现；目标机实际安装/卸载未执行。

## 实现

| 脚本 | 行为 |
|---|---|
| `scripts/v561-uninstall.ps1` | 停止 V5.6.1 进程；删除 runtime 目录；`-RemoveData` 时删除数据根，默认保留 |
| `scripts/v561-reinstall.ps1` | 可选 `-Build`；调用卸载（默认保留数据根）→ 复制运行包 → 调用 `v561-start.ps1` |

安全：

- 拒绝删除盘符根目录；
- 默认保留数据根；
- 不删除模型/用户资产，除非显式 `-RemoveData`；
- 不回退/删除旧 V4.8 安装。

## 使用

```powershell
# 卸载程序，保留数据
pwsh scripts/v561-uninstall.ps1 -DataRoot E:\nai-data

# 重装并启动
pwsh scripts/v561-reinstall.ps1 `
  -RuntimeSource E:\projects\nai\target-windows\build\release `
  -RuntimeDir E:\projects\nai\target-windows\v561-runtime `
  -DataRoot E:\nai-data
```

## 未完成

- 目标机 Windows 实际卸载/重装/启动未执行（当前 Linux/WSL 环境无法运行 exe）。
- 安装包签名/快捷方式/开始菜单项未做；1.3b 与许可/权重来源一起处理。
- 模型/venv 自动安装未做。
