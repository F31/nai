# Qwen 服务重启后验证

用户重启 WSL 并启动 Qwen 后，`GET http://127.0.0.1:8000/health` 返回 ok，服务报告模型为 Qwen3-TTS-12Hz-1.7B-CustomVoice，采样率 24000 Hz、显存约 3.89 GB；`/v1/models` 返回 `qwen3-tts-1.7b-customvoice`。WSL 中服务进程为 uvicorn，已配置模型目录仅发现 1.7B，未发现目标 0.6B。此检查不排除其他位置有模型。

调用本地 `/v1/audio/speech` 一次，声线 serena、中文短句：成功得到 PCM WAV，24 kHz、单声道、16 bit，长度 2.080 秒，端到端耗时 7.765 秒。文件为 `target-windows/v561-qwen-restart-check.wav`；不推流，不修改服务配置。单次耗时不代表稳定性能。

此结果验证当前 1.7B 服务恢复，不能认定 0.6B 通过，也不能替代 FlashHead + Qwen + OBS 的 M0 联合验收。下一步核实 0.6B 的来源、许可及部署路径，再在独立端口验证，避免中断现有 1.7B 服务。
