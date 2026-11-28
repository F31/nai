# Design
先AST核对唯一AutoModel(model_dir=args.model_dir)及main/parse_args位置，新增显式store_true开关，未匹配拒绝。不引入替代模型、VLLM或新依赖。候选文件与原文件hash/镜像/挂载留存；原服务空闲才停止，原容器保留，不修改自动启动。临时候选绑定loopback独立端口、restart=no，原服务停后才启动，测试结束先停候选再恢复原服务并检查健康。不改工作台端点或Key。固定短句/音色和有界探针，先确认FP16实际生效、有限PCM与完成，再考虑人工质量、断连/停止和联合GPU原生三句。少量单进程探测是可行性证据，不满足正式多进程性能/质量门。首token等待与flow上限共同覆盖首PCM缺口，单精度探测不预授800ms预算通过。
