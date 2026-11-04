# Design
确认用户安装包为index-DVNtQIsM.js，React组件与react-dom使用不同实例。Vite resolve.dedupe固定react/react-dom唯一实例，保留CSP不放宽、不改用户代理。
浏览器测试直接加载dist真实产物，检查root中标题/按钮渲染和无JS异常，并验证模拟原生设置异步加载（测试桩只用于测试，不触发真实API）。Tauri smoke由DOM挂载且主机设置显示后的无参数frontend_ready诊断握手判成功；导航完成只代表资源导航，不再直接退出0。标记不含配置/Key且不授予业务状态。
不改消息或配置数据格式，无新契约生成。匹配验证：前端typecheck/build及UI测试、实际Chromium生产bundle、独立壳clippy/测试、修复包页面显示；不重跑知识/升级/平台等无关suite。更新报告/计划并归档。
