//! Current single-room assembly components. No legacy foundation engine startup.
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
