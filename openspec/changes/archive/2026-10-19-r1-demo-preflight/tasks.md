# Tasks

## 1. 演示包与契约
- [x] 1.1 随源码入仓 `demos/manual-demo/`：场景、素材 manifest/config/三 RGBA、`LICENSE`（nai-demo-license）、README；素材通过 schema 校验。
- [x] 1.2 `--preflight` 命令参数与互斥；新增预检模块（scene/knowledge/provider/budget/render/cost/connectivity 逐轴）。
- [x] 1.3 可选受预算连通探针（凭据+显式标志才发真实调用，否则 not_ready）。
## 2. 控制面与桌面
- [x] 2.1 报告写入 `operator-artifacts/preflight.json`；`GET /api/v1/sessions/{id}/preflight` 路由（session.read，未跑=CAPABILITY_NOT_READY）。
- [x] 2.2 桌面 preflight 面板与严格解析，`tests/ui/preflight.test.mjs`。
## 3. 验证与记录
- [x] 3.1 进程级测试：合法 passed / 缺凭据 not_ready / SHA 改动 failed / 无 render not_ready / 探针声明不冒充。
- [x] 3.2 运行匹配检查，更新计划与报告，归档前校验。