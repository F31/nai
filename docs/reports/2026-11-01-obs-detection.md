# OBS 检测分层报告

## 结果

OBS 检测分开报告进程、场景/来源、窗口捕获、声音路由和实际信号。未观察的层保持未知。`audit_obs` 是描述符的纯函数：不打开进程、不读 websocket、不启动录像。进程存在或安装路径存在不得标接入成功。窗口捕获或虚拟摄像头不得当作公共音轨或平台开播。五层均有证据时读作 `Observed`，该状态仍不是播放许可，也不是平台开播。直播准备清单的平台项状态保持 `confirm`。

`cargo test -p nai-kernel --test obs --offline -- --test-threads=1` 12 项通过。重试一次因与 clippy 并行出现 LNK1104，重试后通过。`cargo clippy -p nai-kernel --all-targets --offline -- -D warnings` 通过。`node --test tests/ui/live-readiness.test.mjs` 4 项通过。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。1.3.31 已按这次验收勾选。1.3.10 整项和 1.4 没有勾选。

## 本机 2026-11-01 观察

| 层 | 状态 | 证据 |
| --- | --- | --- |
| 进程 | 已运行 | pid 44740，镜像 `D:\obs-studio\bin\64bit\obs64.exe` |
| 安装树 | 存在 | `D:\obs-studio`；`core\win-capture\win-capture.dll`、`core\win-wasapi\win-wasapi.dll`、`core\obs-websocket\obs-websocket.dll`、`core\win-dshow\win-dshow.dll`、`core\obs-ffmpeg\obs-ffmpeg.dll`、`core\obs-x264\obs-x264.dll` 均存在 |
| 场景/来源 | 未知 | 未读取场景集合 |
| 窗口捕获 | 未知 | 没有标题为 `nai · 公共输出` 的窗口 |
| 声音路由 | 未知 | 未读取 WASAPI 来源 |
| 实际信号 | 未知 | 未读取画面或音量峰值 |
| websocket | 未知 | `127.0.0.1:4455` 本轮无 TCP 观察 |

对应描述符是 `ObsSurface::process_only(44740, "obs64.exe")`。审计为空，就绪为 `Blocked`。该进程存在不是接入成功。

## 没有改变的路径

没有启动录像，没有点击开始直播，没有改写 `D:\obs-studio` 配置。没有公共输出窗口，没有授权音画，没有口型评分。没有真实挑战续租、认证 IPC 或生产许可。直播首页尚未渲染 `liveReadiness`。164 项验收仍全部 pending。
