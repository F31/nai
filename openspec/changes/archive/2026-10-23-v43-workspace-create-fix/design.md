## Decisions
音频停止/nonce为不接受页面路径或凭据的固定原生命令，独立allow-private-audio-control权限只授予main窗口。AppManifest列出所有注册命令，测试比对清单；connect在标connected和frontend_ready之前真实调用停止/读取nonce并校验响应，缺权限不能显示可操作。
创建分提交和读取/激活。收到成功ID即记住待激活项；后续失败给“已创建、待切换”，保留ID供重试读取/激活，禁止自动再次创建。没有收到创建结果时保留原有OutcomeUnknown行为，不重试未知写；旧数据不删除。
## Validation
前端真实编解码模拟顺序：创建成功后激活权限拒绝、下一次恢复只读/切换且创建次数1；首次写失败不假成功。权限精确集合/禁泛化能力静态核对；Windows新壳隔离根烟测实际调用音频命令，真实host创建/重读回归。无需真实API或全仓测试。
