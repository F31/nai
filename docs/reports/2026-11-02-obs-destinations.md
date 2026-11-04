# 虚拟摄像头与 OBS 目的地

## 结果

虚拟摄像头和独立音轨作为可选目的地，分别报告连接、源与真实信号。公共画面仍走窗口。虚拟摄像头只出画面。独立音轨只出声音。未观察保持未知。`audit_dest` 是描述符的纯函数：不打开进程、不枚举 DirectShow 设备、不启动录像。把虚拟摄像头或独立音轨写成公共画面，或把虚拟摄像头写成声音通路，判定为缺陷。工作台公共输出目的地固定为 `window`。直播准备清单的平台项状态保持 `confirm`。2.4.2 已勾选。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。

## 目的地分层

窗口、虚拟摄像头和三条独立音轨（混合、讲解、真人）各自有连接、源与信号三层。空描述符审计为空，就绪为 `WindowPublic`。虚拟摄像头三层均有证据时，公共路径仍是窗口。声称未观察的目的地可用、或 `public_via` 不是窗口，就绪为 `Defective`。1.3.31 的五层 OBS 检测保持原样，本轮不改其 FINDING_TOKENS。

工作台 `CharacterPublicOutput.destination` 为 `window`。桌面视图增加 `publicDestination`。公共输出页增加目的地说明：窗口捕获仍是公共画面，虚拟摄像头和独立音轨不替代窗口。

## 校验

`cargo test -p nai-kernel --test obs_dest --offline -- --test-threads=1`：10 项通过。`cargo test -p nai-kernel --test obs --offline -- --test-threads=1`：12 项通过。`cargo test -p nai-media-core --offline --lib character_workbench`：7 项通过。`cargo clippy -p nai-kernel --all-targets --offline -- -D warnings` 通过。`cargo clippy -p nai-media-core --offline -- -D warnings` 通过。嵌套工作区 `cargo check --offline --manifest-path apps/workbench-desktop/Cargo.toml --bin nai-workbench-desktop` 通过。`node --test tests/ui/live-readiness.test.mjs`：7 项通过。`tsc --noEmit -p apps/workbench-desktop/tsconfig.json` 通过。媒体核心库测试链接出现一次 LNK1104，重试后通过，未改代码。本轮未打开实际桌面窗口，未启动 OBS 录像。

## 仍开放的项

2.4.3 外部渲染服务仍开放。2.5 与 3.x 仍开放。OBS 自带「开始虚拟摄像头」，nai 不另装第三方虚拟摄像头驱动，也不自研驱动（基线 §9.1）。本轮没有启动这项 OBS 输出，也没有把它的运行信号写成已观察目的地。`win-dshow.dll` 是把摄像头采进 OBS 的捕获插件，安装树存在不能当作虚拟摄像头已可用。164 项验收仍全部 pending。
