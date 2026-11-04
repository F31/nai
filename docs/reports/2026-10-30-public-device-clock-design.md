# 公共输出设备时钟接入设计

核对当前工作台公共输出：`nai-audio-cpal::open_controlled` 提供的是提交样本数，`preload.rs` 依此推进画面和字幕；现有 output-keeper 的 WASAPI 设备时钟属于另一个音频会话，不能跨会话直接当作公共输出播放位置。本轮在 v49 OpenSpec 记录同一受控会话内硬件位置、身份与代次、提交上界、失联停止、迟帧处理及契约生成顺序；保留实际播放、声学完成和提交之间的区别。

`pnpm exec openspec validate v49-realistic-avatar --strict` 通过。仅完成设计核对，尚未修改契约、实现设备后端或生产接线，也未完成真实音画与 OBS 验证。下一步按规格先定义并生成 workbench 契约，再实施同会话 WASAPI 适配和定向故障验证；不可将现有设备测试当作公共输出通过。
