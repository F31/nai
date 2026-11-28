//! Current single-room assembly components. No legacy foundation engine startup.
// 设备面模块（外部 PCM / 直播控制 / 待机画面 / 呈现 / 时钟 / 就绪计划 / 头像流）的
// 真实消费者都在 `#[cfg(windows)]` 代码或 Windows-only bin 里；Linux 门控只编译
// 平台中立单测，于是这些项在该平台必然报 dead_code/unused_imports。这是平台差异，
// 不是死代码判据退化：Windows 侧仍以 -D warnings 全量检查。把豁免只在 not(windows)
// 侧收敛到 crate 根一处，Windows 侧判据一字不改。
#![cfg_attr(not(windows), allow(dead_code, unused_imports))]
pub mod v561_audio;
pub mod v561_audition_control;
pub mod v561_avatar_batch;
pub mod v561_avatar_frames;
pub mod v561_avatar_input;
pub mod v561_avatar_metrics;
pub mod v561_avatar_preview;
mod v561_avatar_process;
pub mod v561_avatar_raw;
pub mod v561_avatar_stream;
pub mod v561_avatar_worker;
#[cfg(windows)]
pub mod v561_continuous_session;
pub mod v561_external_pcm;
pub mod v561_frame_feed;
pub mod v561_gpu_producer;
pub mod v561_live_control;
pub mod v561_narration;
#[cfg(windows)]
pub mod v561_obs_encoder;
pub mod v561_obs_timeline;
pub mod v561_pcm_gate;
pub mod v561_pcm_source;
pub mod v561_prepared_asset;
pub mod v561_prepared_plan;
pub mod v561_presentation;
pub mod v561_question;
pub mod v561_reply_llm;
pub(crate) mod v561_reply_playback;
pub mod v561_script_source;
mod v561_standby;
pub mod v561_stream_audio;
pub mod v561_stream_clip;
pub mod v561_stream_clock;
#[cfg(windows)]
pub mod v561_stream_output;
#[cfg(windows)]
pub mod v561_stream_script;
pub mod v561_stream_synthesis;
pub mod v561_task_control;
pub mod v561_tasks;
pub mod v561_voice_preparation;
#[cfg(windows)]
mod v561_window_witness;
