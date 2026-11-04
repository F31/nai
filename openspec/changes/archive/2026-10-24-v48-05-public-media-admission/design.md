# 设计
PublicMediaBundle只包含资源身份、SHA256和格式元数据，不放路径/文本/PCM/帧。canonical hash排除自身approved_sha256，所有其余字段绑定审批；拒绝缺轴、重复身份、格式超预算和未知协议。独立PublicMediaAuthority端口提供可信当前身份、事实、审批/guard、资源许可及有效期；无生产实现时不产生公共许可，tests中的权威夹具不进入运行包。

PublicMediaPermit使用现有StopFence原子P0，不依赖普通模型/DB锁。准入和续检都对比可信状态，最晚500ms，任何过期/撤权/身份或内容变化后拒绝；失败后不可旧消息重新激活。媒体消费必须继续复检，本轮不把此许可接入待机健康协议或私有音频。

整句语音利用已有价格未知字段，plan增加estimate_known状态；未知价格按用户确认max_cost全额预留，状态参与批准hash，费用结算沿既有TTS unknown规则。移除前端priceKnown筛选，但保留模型启用/凭据、文字/音色/费用确认及私有scope。相关内核/真实SQLite/前端和生成检查，Windows构建更新；无真实付费请求。
