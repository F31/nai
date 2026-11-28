//! Application composition for a reusable native single-room stream.
use crate::{
    v561_avatar_stream::Resident,
    v561_avatar_worker::Config,
    v561_external_pcm,
    v561_pcm_gate::PcmGate,
    v561_stream_audio, v561_stream_clock,
    v561_stream_synthesis::{self, MediaSink},
};
use minifb::{Key, MouseButton, Window, WindowOptions};
use nai_contracts::{
    v561_media::{ClockState, PlaybackClockObservation},
    v561_media_validation::FrameLimits,
    workbench::WorkbenchError as Error,
};
use nai_media_core::v561_frame_scheduler::{Frame, Offer, OwnedVerifiedFrame, Scheduler};
use nai_runtime_core::foundation::Operation;
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

const DEVICE_PCM_BLOCK_SAMPLES: usize = 4800;
const DEVICE_PCM_BLOCK_BYTES: usize = DEVICE_PCM_BLOCK_SAMPLES * 2;

fn copy_bgra_to_window(screen: &mut [u32], pixels: &[u8]) {
    for (dst, pixel) in screen.iter_mut().zip(pixels.chunks_exact(4)) {
        *dst = u32::from_le_bytes([pixel[0], pixel[1], pixel[2], 0]);
    }
}
struct Sink {
    gate: PcmGate,
    audio: v561_external_pcm::Sender,
    scheduler: Arc<Mutex<Option<Scheduler>>>,
    generation: u64,
    epoch: u64,
    capacity: usize,
    limits: FrameLimits,
    frames: u64,
    pending: Vec<u8>,
    pcm_observer: Option<PcmObserver>,
}
struct PcmObserver {
    sender: mpsc::SyncSender<SelectedPcm>,
    operation: Operation,
    next_sample: u64,
    active: Arc<AtomicBool>,
}
impl PcmObserver {
    fn publish(&mut self, epoch: u64, generation: u64, bytes: &[u8]) -> bool {
        let start_sample = self.next_sample;
        self.next_sample += bytes.len() as u64 / 2;
        if !self.active.load(Ordering::Acquire) {
            return false;
        }
        let sent = self
            .sender
            .try_send(SelectedPcm {
                epoch,
                generation,
                start_sample,
                bytes: zeroize::Zeroizing::new(bytes.to_vec()),
                operation: self.operation.clone(),
                active: self.active.clone(),
            })
            .is_ok();
        if !sent {
            self.active.store(false, Ordering::Release);
        }
        sent
    }
}
enum InputSource {
    Provider(nai_llm_openai::cosyvoice::Config, String),
    Prepared(Box<crate::v561_stream_clip::Clip>),
    Asset(
        Arc<crate::v561_prepared_asset::Asset>,
        [u8; 32],
        Option<Instant>,
        Option<crate::v561_prepared_asset::VerifiedFrames>,
    ),
}
struct PlaybackSource {
    samples: u64,
    first_pcm_ms: u128,
    first_frame_ms: Option<u128>,
    adapter_first_pcm_ms: Option<u128>,
}
impl From<v561_stream_synthesis::Production> for PlaybackSource {
    fn from(p: v561_stream_synthesis::Production) -> Self {
        Self {
            samples: p.provider.samples,
            first_pcm_ms: p.first_pcm_ms,
            first_frame_ms: p.first_frame_ms,
            adapter_first_pcm_ms: Some(p.provider.first_pcm_ms),
        }
    }
}
impl InputSource {
    fn maximum(&self) -> u64 {
        match self {
            Self::Provider(config, _) => config.max_samples,
            Self::Prepared(clip) => clip.maximum,
            Self::Asset(asset, ..) => asset.samples(),
        }
    }
    fn origin(&self) -> Option<Instant> {
        match self {
            Self::Provider(..) => None,
            Self::Prepared(clip) => Some(clip.facts.origin),
            Self::Asset(_, _, origin, _) => *origin,
        }
    }
}
impl Sink {
    fn drain(&mut self) -> Result<(), Error> {
        while let Some(mut bytes) = self
            .gate
            .take(
                (DEVICE_PCM_BLOCK_SAMPLES as u64).min(self.gate.maximum()),
                false,
            )
            .map_err(|_| Error::ProtocolError)?
        {
            self.pending.extend_from_slice(&bytes);
            zeroize::Zeroize::zeroize(&mut bytes);
            while self.pending.len() >= DEVICE_PCM_BLOCK_BYTES {
                self.audio.push(&self.pending[..DEVICE_PCM_BLOCK_BYTES])?;
                if let Some(observer) = &mut self.pcm_observer
                    && !observer.publish(
                        self.epoch,
                        self.generation,
                        &self.pending[..DEVICE_PCM_BLOCK_BYTES],
                    )
                {
                    self.pcm_observer = None;
                }
                self.pending.drain(..DEVICE_PCM_BLOCK_BYTES);
            }
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<(), Error> {
        self.drain()?;
        if self.gate.submitted() != self.gate.received() {
            return Err(Error::ProtocolError);
        }
        if !self.pending.is_empty() {
            self.audio.push(&self.pending)?;
            if let Some(observer) = &mut self.pcm_observer
                && !observer.publish(self.epoch, self.generation, &self.pending)
            {
                self.pcm_observer = None;
            }
            self.pending.clear();
        }
        self.audio.finish()
    }
}
impl Drop for Sink {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.pending);
    }
}
impl MediaSink for Sink {
    fn pcm(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.gate.append(bytes).map_err(|_| Error::ProtocolError)
    }
    fn frame(&mut self, frame: Frame) -> Result<(), Error> {
        let verified =
            nai_media_core::v561_frame_scheduler::OwnedVerifiedFrame::verify(frame, self.limits)
                .map_err(|_| Error::ProtocolError)?;
        self.verified_frame(verified)
    }
    fn verified_frame(
        &mut self,
        frame: nai_media_core::v561_frame_scheduler::OwnedVerifiedFrame,
    ) -> Result<(), Error> {
        let source = frame.metadata().source_sample.ok_or(Error::ProtocolError)?;
        {
            let mut slot = self.scheduler.lock().map_err(|_| Error::IoFailed)?;
            if slot.is_none() {
                *slot = Some(
                    Scheduler::new(
                        frame.metadata().pool_id.clone(),
                        self.epoch,
                        self.generation,
                        self.limits,
                        self.capacity,
                        2400,
                    )
                    .map_err(|_| Error::InvalidArgument)?,
                );
            }
            if slot
                .as_mut()
                .ok_or(Error::ProtocolError)?
                .offer_verified(frame)
                .map_err(|_| Error::ResourceExhausted)?
                != Offer::Queued
            {
                return Err(Error::ProtocolError);
            }
        }
        self.gate.frame(source).map_err(|_| Error::ProtocolError)?;
        self.frames += 1;
        self.drain()
    }
    fn verified_eos(&mut self, samples: u64, digest: &[u8]) -> Result<(), Error> {
        self.gate
            .seal(samples, digest)
            .map_err(|_| Error::ProtocolError)?;
        self.drain()?;
        Ok(())
    }
}

/// Prepared visual worker only; TTS and device capability remain independently checked.
pub struct PreparedSession {
    owner: Option<Resident>,
    config: Config,
    next: u64,
    content_context: [u8; 32],
    output_observer: Option<OutputSender>,
    pub ready_ms: u64,
    pub preparation_ms: u128,
    pub worker_pid: u32,
}
struct OutputSender {
    frames: mpsc::SyncSender<SelectedFrame>,
    pcm: mpsc::SyncSender<SelectedPcm>,
    completed: mpsc::SyncSender<u64>,
}
pub struct OutputObserver {
    pub frames: mpsc::Receiver<SelectedFrame>,
    pub pcm: mpsc::Receiver<SelectedPcm>,
    pub(crate) completed: mpsc::Receiver<u64>,
}
pub struct SelectedPcm {
    epoch: u64,
    generation: u64,
    start_sample: u64,
    bytes: zeroize::Zeroizing<Vec<u8>>,
    operation: Operation,
    active: Arc<AtomicBool>,
}
impl SelectedPcm {
    /// Returns PCM admitted to the device input. Use the authenticated clock
    /// separately to determine actual playback position.
    pub fn into_parts(self) -> Option<(u64, u64, u64, zeroize::Zeroizing<Vec<u8>>)> {
        if !self.active.load(Ordering::Acquire) {
            return None;
        }
        self.operation.check().ok()?;
        Some((self.epoch, self.generation, self.start_sample, self.bytes))
    }
}
pub struct SelectedFrame {
    frame: OwnedVerifiedFrame,
    clock: PlaybackClockObservation,
    operation: Operation,
    active: Arc<AtomicBool>,
    expires: Instant,
}
impl SelectedFrame {
    /// This grants access to a snapshot, not playback permission.
    pub fn into_fresh_parts(self) -> Option<(OwnedVerifiedFrame, PlaybackClockObservation)> {
        (self.active.load(Ordering::Acquire)
            && self.operation.check().is_ok()
            && Instant::now() < self.expires)
            .then_some((self.frame, self.clock))
    }
}
pub(crate) struct ScriptOutput<'a> {
    pub window: &'a mut Window,
    pub audio: &'a crate::v561_continuous_session::Handle,
    pub standby: Option<&'a crate::v561_standby::View>,
}
impl PreparedSession {
    pub(crate) fn gpu_script(
        &mut self,
        pipeline: Arc<Mutex<crate::v561_script_source::Pipeline>>,
        root: Operation,
        count: u32,
        trace: &Path,
    ) -> Result<crate::v561_gpu_producer::Producer, Error> {
        let owner = self.owner.take().ok_or(Error::CapabilityUnavailable)?;
        Ok(crate::v561_gpu_producer::Producer::start(
            owner,
            pipeline,
            root,
            self.next,
            count,
            self.config.temp_dir.clone(),
            u64::from(self.config.width) * u64::from(self.config.height) * 4,
            trace.to_owned(),
        ))
    }
    pub(crate) fn restore_script(&mut self, owner: Resident, epoch: u64) {
        self.next = self.next.max(epoch);
        if owner.available() {
            self.owner = Some(owner);
        }
    }
    pub fn script_window(&self) -> Result<Window, Box<dyn std::error::Error>> {
        self.output_window(true)
    }
    pub(crate) fn standby_window(
        &self,
        view: Option<&crate::v561_standby::View>,
    ) -> Result<Window, Box<dyn std::error::Error>> {
        let mut window = self.output_window(false)?;
        let mut screen = vec![0; (self.config.width * self.config.height) as usize];
        crate::v561_standby::fill(&mut screen, view);
        window.update_with_buffer(
            &screen,
            self.config.width as usize,
            self.config.height as usize,
        )?;
        Ok(window)
    }
    fn output_window(&self, topmost: bool) -> Result<Window, Box<dyn std::error::Error>> {
        let mut window = Window::new(
            "nai · 数字人讲解",
            self.config.width as usize,
            self.config.height as usize,
            WindowOptions {
                topmost,
                ..WindowOptions::default()
            },
        )?;
        window.set_position(80, 80);
        window.set_target_fps(100);
        Ok(window)
    }
    pub fn prepare(
        config: Config,
        worker: &Path,
        dir: &Path,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let operation =
            Operation::maintenance(config.maintenance_timeout_ms).map_err(|e| e.as_str_name())?;
        Self::prepare_in(config, worker, dir, &operation)
    }
    pub fn prepare_in(
        config: Config,
        worker: &Path,
        dir: &Path,
        operation: &Operation,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(dir)?;
        let began = Instant::now();
        let owner = Resident::start(&config, worker, operation, &dir.join("worker.log"))
            .map_err(|e| e.as_str_name())?;
        let ready_ms = owner.ready.elapsed_ms;
        let preparation_ms = began.elapsed().as_millis();
        let worker_pid = owner.process_id();
        use ring::rand::SecureRandom;
        let mut content_context = [0; 32];
        ring::rand::SystemRandom::new()
            .fill(&mut content_context)
            .map_err(|_| "CONTENT_CONTEXT_FAILED")?;
        Ok(Self {
            owner: Some(owner),
            config,
            next: 0,
            content_context,
            output_observer: None,
            ready_ms,
            preparation_ms,
            worker_pid,
        })
    }
    pub fn available(&self) -> bool {
        self.owner.as_ref().is_some_and(Resident::available)
    }
    /// Observe the next playback only. A PCM overrun revokes both streams
    /// without blocking the actual playback path.
    pub fn observe_output(&mut self) -> OutputObserver {
        let (frame_sender, frames) = mpsc::sync_channel(2);
        let pcm_capacity = (v561_stream_audio::DEFAULT_CAPACITY_SAMPLES as usize)
            .div_ceil(DEVICE_PCM_BLOCK_SAMPLES);
        let (pcm_sender, pcm) = mpsc::sync_channel(pcm_capacity);
        let (completion_sender, completed) = mpsc::sync_channel(1);
        self.output_observer = Some(OutputSender {
            frames: frame_sender,
            pcm: pcm_sender,
            completed: completion_sender,
        });
        OutputObserver {
            frames,
            pcm,
            completed,
        }
    }
    pub(crate) fn liveness(&self) -> Option<Arc<std::sync::atomic::AtomicBool>> {
        self.owner.as_ref().map(Resident::liveness)
    }
    pub fn content_identity(
        &self,
        tts: &nai_llm_openai::cosyvoice::Config,
        text: &str,
        op: &Operation,
    ) -> Result<[u8; 32], Error> {
        crate::v561_prepared_asset::identity(&self.content_context, &self.config, tts, text, op)
    }
    pub fn content_keys(
        &self,
        tts: &nai_llm_openai::cosyvoice::Config,
        text: &str,
        op: &Operation,
    ) -> Result<Vec<[u8; 32]>, Error> {
        tts.lookahead
            .split(text)?
            .iter()
            .map(|part| self.content_identity(tts, part, op))
            .collect()
    }
    pub fn prepare_plan(
        &mut self,
        tts: &nai_llm_openai::cosyvoice::Config,
        text: &str,
        policy: crate::v561_prepared_asset::Policy,
        dir: &Path,
        op: Operation,
    ) -> Result<Arc<crate::v561_prepared_plan::Plan>, Box<dyn std::error::Error>> {
        policy.validate().map_err(|e| e.as_str_name())?;
        let parts = tts.lookahead.split(text).map_err(|e| e.as_str_name())?;
        if parts.is_empty() || parts.len() > policy.max_entries {
            return Err("CONTENT_PLAN_LIMIT".into());
        }
        let keys = self
            .content_keys(tts, text, &op)
            .map_err(|e| e.as_str_name())?;
        let mut assets = Vec::with_capacity(parts.len());
        let mut used = 0u64;
        let mut samples = 0u64;
        for (index, part) in parts.iter().enumerate() {
            op.check().map_err(|e| e.as_str_name())?;
            let remaining = policy
                .max_total_bytes
                .checked_sub(used)
                .ok_or("CONTENT_PLAN_LIMIT")?;
            if remaining < 4194304 {
                return Err("CONTENT_PLAN_LIMIT".into());
            }
            let mut leaf_policy = policy;
            leaf_policy.max_bytes = leaf_policy.max_bytes.min(remaining);
            let sentence_started = Instant::now();
            let asset = self.prepare_content(
                tts,
                part,
                leaf_policy,
                &dir.join(format!("sentence-{}", index + 1)),
                op.clone(),
            ).inspect_err(|error| {
                // Diagnostics cannot replace the original failure or grant publication.
                if let Ok(file) = std::fs::File::create_new(dir.join("sentence-failure.json")) {
                    let _ = serde_json::to_writer_pretty(file, &serde_json::json!({
                        "stage":"sentence_generation", "sentence":index+1,
                        "elapsed_ms":sentence_started.elapsed().as_millis(),
                        "reason":crate::v561_prepared_plan::safe_failure_reason(error.as_ref()),
                        "operation_error":op.check().err().map(|e|e.as_str_name()),
                        "published":false
                    }));
                }
            })?;
            used = used
                .checked_add(asset.bytes())
                .ok_or("CONTENT_PLAN_LIMIT")?;
            samples = samples
                .checked_add(asset.samples())
                .ok_or("CONTENT_PLAN_LIMIT")?;
            if samples > 480000 {
                return Err("CONTENT_PLAN_LIMIT".into());
            }
            assets.push(asset);
        }
        if self
            .content_keys(tts, text, &op)
            .map_err(|e| e.as_str_name())?
            != keys
        {
            return Err("CONTENT_INPUT_CHANGED".into());
        }
        let plan = crate::v561_prepared_plan::Plan::new(text, assets, policy, &op)
            .map_err(|e| e.as_str_name())?;
        if !plan.matches(text, &keys) {
            return Err("CONTENT_INPUT_CHANGED".into());
        }
        serde_json::to_writer_pretty(
            std::fs::File::create_new(dir.join("plan.json"))?,
            &plan.manifest(),
        )?;
        op.check().map_err(|e| e.as_str_name())?;
        Ok(Arc::new(plan))
    }
    pub fn prepare_content(
        &mut self,
        tts: &nai_llm_openai::cosyvoice::Config,
        text: &str,
        policy: crate::v561_prepared_asset::Policy,
        dir: &Path,
        op: Operation,
    ) -> Result<Arc<crate::v561_prepared_asset::Asset>, Box<dyn std::error::Error>> {
        if !self.available() {
            return Err("NOT_PREPARED".into());
        }
        op.check().map_err(|e| e.as_str_name())?;
        let key = self
            .content_identity(tts, text, &op)
            .map_err(|e| e.as_str_name())?;
        self.next = self.next.checked_add(1).ok_or("epoch exhausted")?;
        std::fs::create_dir_all(dir)?;
        let began = Instant::now();
        let mut writer = crate::v561_prepared_asset::Builder::new(
            dir,
            key,
            self.next,
            self.config.width,
            self.config.height,
            policy,
            op.clone(),
        )
        .map_err(|e| e.as_str_name())?;
        let production = v561_stream_synthesis::generate_media(
            self.owner.as_mut().ok_or("NOT_PREPARED")?,
            tts,
            text,
            self.next,
            self.next,
            op.clone(),
            &mut writer,
        )
        .map_err(|e| e.as_str_name())?;
        if self
            .content_identity(tts, text, &op)
            .map_err(|e| e.as_str_name())?
            != key
        {
            return Err("CONTENT_INPUT_CHANGED".into());
        }
        let asset = writer.finish(&production).map_err(|e| e.as_str_name())?;
        op.check().map_err(|e| e.as_str_name())?;
        serde_json::to_writer_pretty(
            std::fs::File::create_new(dir.join("manifest.json"))?,
            &asset.manifest(),
        )?;
        serde_json::to_writer_pretty(
            std::fs::File::create_new(dir.join("preparation.json"))?,
            &serde_json::json!({"scope":"private_session_asset_preparation","model_api_calls":1,"samples":asset.samples(),"elapsed_ms":began.elapsed().as_millis(),"worker_pid":self.worker_pid,"source_epoch":self.next}),
        )?;
        Ok(Arc::new(asset))
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn play_prepared_in(
        &mut self,
        asset: Arc<crate::v561_prepared_asset::Asset>,
        key: [u8; 32],
        audio: &Path,
        dir: &Path,
        prefill: u64,
        op: Operation,
        origin: Instant,
        verified: Option<crate::v561_prepared_asset::VerifiedFrames>,
        output: ScriptOutput<'_>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        if !asset.matches(&key) {
            return Err("CONTENT_NOT_PREPARED".into());
        }
        let prefill = prefill.min(asset.samples());
        self.play_inner(
            InputSource::Asset(asset, key, Some(origin), verified),
            audio,
            dir,
            prefill,
            0,
            false,
            Some(op),
            false,
            Some(output.window),
            Some(output.audio.clone()),
            output.standby,
        )
    }
    pub fn play_prepared(
        &mut self,
        asset: Arc<crate::v561_prepared_asset::Asset>,
        key: [u8; 32],
        audio: &Path,
        dir: &Path,
        prefill: u64,
        op: Operation,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        if !asset.matches(&key) {
            return Err("CONTENT_NOT_PREPARED".into());
        }
        let prefill = prefill.min(asset.samples());
        self.play_inner(
            InputSource::Asset(asset, key, None, None),
            audio,
            dir,
            prefill,
            0,
            false,
            Some(op),
            false,
            None,
            None,
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn play(
        &mut self,
        tts: nai_llm_openai::cosyvoice::Config,
        text: String,
        audio_exe: &Path,
        dir: &Path,
        prefill: u64,
        stop_ms: u64,
        manual: bool,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        self.play_inner(
            InputSource::Provider(tts, text),
            audio_exe,
            dir,
            prefill,
            stop_ms,
            manual,
            None,
            true,
            None,
            None,
            None,
        )
    }
    pub fn play_controlled(
        &mut self,
        tts: nai_llm_openai::cosyvoice::Config,
        text: String,
        audio_exe: &Path,
        dir: &Path,
        prefill: u64,
        operation: Operation,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        self.play_inner(
            InputSource::Provider(tts, text),
            audio_exe,
            dir,
            prefill,
            0,
            false,
            Some(operation),
            false,
            None,
            None,
            None,
        )
    }
    pub(crate) fn play_source(
        &mut self,
        source: crate::v561_stream_clip::Clip,
        audio_exe: &Path,
        dir: &Path,
        prefill: u64,
        operation: Operation,
        output: ScriptOutput<'_>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let source = InputSource::Prepared(Box::new(source));
        let prefill = prefill.min(source.maximum());
        self.play_inner(
            source,
            audio_exe,
            dir,
            prefill,
            0,
            false,
            Some(operation),
            false,
            Some(output.window),
            Some(output.audio.clone()),
            output.standby,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn play_inner(
        &mut self,
        input: InputSource,
        audio_exe: &Path,
        dir: &Path,
        prefill: u64,
        stop_ms: u64,
        manual: bool,
        parent: Option<Operation>,
        diagnostic: bool,
        shared_window: Option<&mut Window>,
        shared_audio: Option<crate::v561_continuous_session::Handle>,
        standby: Option<&crate::v561_standby::View>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let play_entry = Instant::now();
        let output_observer = self.output_observer.take();
        let prepared_asset = matches!(&input, InputSource::Asset(..));
        let asset_manifest = if let InputSource::Asset(asset, ..) = &input {
            Some(asset.manifest())
        } else {
            None
        };
        if !self.available() && !matches!(&input, InputSource::Prepared(_)) {
            return Err("avatar not prepared".into());
        }
        if stop_ms > 20000 {
            return Err("stop bound".into());
        }
        let config = self.config.clone();
        std::fs::create_dir_all(dir)?;
        let pixel_bytes = u64::from(config.width) * u64::from(config.height) * 4;
        let mut pixel_witness =
            crate::v561_window_witness::Witness::new(dir.join("window-pixels.json"));
        if pixel_bytes == 0 {
            return Err("geometry".into());
        }
        let capacity = (268435456 / pixel_bytes).min(96) as usize;
        if capacity == 0 || prefill.div_ceil(960) > capacity as u64 {
            return Err("prefill pixel budget".into());
        }
        let epoch = match &input {
            InputSource::Prepared(clip) => clip.epoch,
            _ => self.next.checked_add(1).ok_or("identity exhausted")?,
        };
        self.next = self.next.max(epoch);
        let cached_observer = match &input {
            InputSource::Prepared(clip) => Some(clip.observer()),
            _ => None,
        };
        let width = config.width;
        let height = config.height;
        let mut owned_window;
        let window = match shared_window {
            Some(window) => window,
            None => {
                owned_window = self.script_window()?;
                &mut owned_window
            }
        };
        let mut screen = vec![0u32; (width * height) as usize];
        if manual {
            window.set_title("nai · 准备完成：点击绿色窗口开始音画（也可按空格）；Esc取消");
            screen.fill(0x173d35);
            std::fs::write(
                dir.join("awaiting-start.json"),
                b"{\"manual_start\":true,\"model_api_calls\":0}",
            )?;
            let waiting = Instant::now();
            while window.is_open()
                && !window.is_key_down(Key::Space)
                && !window.get_mouse_down(MouseButton::Left)
            {
                if window.is_key_down(Key::Escape)
                    || waiting.elapsed()
                        > Duration::from_millis(config.maintenance_timeout_ms.into())
                {
                    return Err("manual start cancelled or timed out; no model called".into());
                }
                window.update_with_buffer(&screen, width as usize, height as usize)?;
            }
            if !window.is_open() {
                return Err("manual window closed; no model called".into());
            }
            screen.fill(0);
            window.set_title("nai · 同源流式音画验收：正在准备本句音画");
        } else if diagnostic {
            screen.fill(0x173d35);
            window.set_title("nai · 音画验收：5秒后自动开始，请观察此置顶窗口");
            let until = Instant::now() + Duration::from_secs(5);
            while Instant::now() < until {
                if !window.is_open() || window.is_key_down(Key::Escape) {
                    return Err("window closed before synthesis; no model called".into());
                }
                window.update_with_buffer(&screen, width as usize, height as usize)?;
            }
            screen.fill(0);
            window.set_title("nai · 同源流式音画验收：正在准备本句音画");
        }
        let operation = match parent {
            Some(value) => value,
            None => Operation::model_task(45000).map_err(|e| e.as_str_name())?,
        };
        operation.check().map_err(|e| e.as_str_name())?;
        if standby.is_some() {
            crate::v561_standby::fill(&mut screen, standby);
            window.update_with_buffer(&screen, width as usize, height as usize)?;
        }
        struct Cancel(Operation);
        impl Drop for Cancel {
            fn drop(&mut self) {
                self.0.cancel();
            }
        }
        let _cancel = Cancel(operation.clone());
        crate::v561_presentation::require_render_area(window.get_size(), &operation)?;
        let maximum = input.maximum();
        let timeline = input.origin();
        let gate =
            PcmGate::new(maximum, prefill, operation.clone()).map_err(|_| "prefill config")?;
        let (audio_tx, audio_source) = v561_external_pcm::channel(maximum, 16, operation.clone())
            .map_err(|e| e.as_str_name())?;
        let (clock_tx, clock_rx) = v561_stream_clock::channel();
        let audio_exe = audio_exe.to_path_buf();
        let audio_op = operation.clone();
        let audio_thread = std::thread::spawn(move || {
            let result = if let Some(handle) = shared_audio {
                handle.play(audio_source, epoch, audio_op.clone(), clock_tx)
            } else {
                v561_stream_audio::play_external_with_clock(
                    audio_source,
                    &audio_exe,
                    audio_op.clone(),
                    clock_tx,
                    epoch,
                    v561_stream_audio::Options::default(),
                )
            };
            if result.is_err() {
                audio_op.cancel();
            }
            result
        });
        let device_wait_began = Instant::now();
        let prepared = clock_rx.recv_timeout(Duration::from_secs(3))?;
        let device_prepare_wait_us = device_wait_began.elapsed().as_micros();
        if !prepared.fresh() || prepared.observation().state != ClockState::Unavailable as i32 {
            return Err("device not prepared".into());
        }
        let generation = prepared.observation().device_generation;
        let scheduler = Arc::new(Mutex::new(None));
        let gpu_scheduler = scheduler.clone();
        let gpu_op = operation.clone();
        let observers = output_observer.map(|sender| {
            let active = Arc::new(AtomicBool::new(true));
            (
                sender.frames,
                PcmObserver {
                    sender: sender.pcm,
                    operation: operation.clone(),
                    next_sample: 0,
                    active: active.clone(),
                },
                active,
                sender.completed,
            )
        });
        let (frame_observer, pcm_observer, observer_active, observer_completed) = match observers {
            Some((frames, pcm, active, completed)) => {
                (Some(frames), Some(pcm), Some(active), Some(completed))
            }
            None => (None, None, None, None),
        };
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let mut owner = self.owner.take();
        let gpu_thread = std::thread::spawn(move || {
            let result = (|| {
                let mut sink = Sink {
                    gate,
                    audio: audio_tx,
                    scheduler: gpu_scheduler.clone(),
                    generation,
                    epoch,
                    capacity,
                    limits: FrameLimits {
                        slots: 3,
                        max_width: width,
                        max_height: height,
                        max_payload_bytes: pixel_bytes,
                        max_sample_rate: 24000,
                    },
                    frames: 0,
                    pending: Vec::new(),
                    pcm_observer,
                };
                let p = match input {
                    InputSource::Provider(tts, text) => v561_stream_synthesis::generate_media(
                        owner.as_mut().ok_or(Error::CapabilityUnavailable)?,
                        &tts,
                        text.trim(),
                        epoch,
                        epoch,
                        gpu_op.clone(),
                        &mut sink,
                    )?
                    .into(),
                    InputSource::Prepared(clip) => (*clip)
                        .replay(&mut sink, &gpu_op, || {
                            loop {
                                gpu_op.check()?;
                                if gpu_scheduler
                                    .lock()
                                    .map_err(|_| Error::IoFailed)?
                                    .as_ref()
                                    .is_none_or(Scheduler::has_capacity)
                                {
                                    return Ok(());
                                }
                                std::thread::sleep(Duration::from_millis(5));
                            }
                        })?
                        .into(),
                    InputSource::Asset(asset, key, _, verified) => {
                        let replay = asset.replay_with_verified(
                            &key,
                            epoch,
                            &gpu_op,
                            &mut sink,
                            || {
                                loop {
                                    gpu_op.check()?;
                                    if gpu_scheduler
                                        .lock()
                                        .map_err(|_| Error::IoFailed)?
                                        .as_ref()
                                        .is_none_or(Scheduler::has_capacity)
                                    {
                                        return Ok(());
                                    }
                                    std::thread::sleep(Duration::from_millis(5));
                                }
                            },
                            verified,
                        )?;
                        PlaybackSource {
                            samples: replay.samples,
                            first_pcm_ms: replay.first_pcm_ms,
                            first_frame_ms: replay.first_frame_ms,
                            adapter_first_pcm_ms: None,
                        }
                    }
                };
                sink.finish()?;
                Ok::<_, Error>((p, sink.frames))
            })();
            let successful = result.is_ok();
            if !successful {
                gpu_op.cancel();
            }
            let _ = done_tx.send(result);
            owner
        });
        let mut shown = 0u64;
        let mut selected_count = 0u64;
        let mut last_submitted = None;
        let mut pixels_ready = false;
        let mut selected = None;
        let mut observed_sequence = None;
        let mut observed_origin = None;
        let mut stopped = false;
        let mut running = None;
        let mut latest = None;
        let mut result = None;
        let mut requested_stop = false;
        // Local diagnostics only: aggregate counters keep memory bounded and do
        // not change media admission, clock selection or playback deadlines.
        let mut pixel_conversion_us = 0u128;
        let mut max_pixel_conversion_us = 0u128;
        let mut window_submit_us = 0u128;
        let mut max_window_submit_us = 0u128;
        let mut window_submissions = 0u64;
        let mut standby_paints = 0u64;
        let loop_result = (|| -> Result<(), Box<dyn std::error::Error>> {
            loop {
                if !window.is_open() {
                    requested_stop = true;
                    operation.cancel();
                    break;
                }
                if operation.check().is_err() {
                    if requested_stop {
                        break;
                    }
                    return Err("task cancelled or expired".into());
                }
                loop {
                    let event = match clock_rx.try_recv() {
                        Ok(event) => event,
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) if stopped => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            return Err("device clock disconnected".into());
                        }
                    };
                    let clock = event.observation();
                    if clock.state == ClockState::Stopped as i32 {
                        if clock.epoch != epoch || clock.device_generation != generation {
                            return Err("clock rejected".into());
                        }
                        stopped = true;
                        standby_paints +=
                            u64::from(crate::v561_standby::fill(&mut screen, standby));
                        pixels_ready = false;
                        continue;
                    }
                    if !event.fresh() {
                        return Err("stale device clock".into());
                    }
                    if clock.played_sample.unwrap_or(0) > 0 {
                        running.get_or_insert_with(Instant::now);
                    }
                    if let Some(s) = scheduler.lock().map_err(|_| "scheduler poisoned")?.as_mut() {
                        if frame_observer.is_some()
                            && observed_sequence.is_none()
                            && observed_origin.is_none()
                        {
                            observed_origin = s.copy_verified_origin();
                        }
                        if let Some(frame) = s.present(clock).map_err(|_| "clock rejected")? {
                            if selected != frame.metadata.sequence {
                                if diagnostic || manual {
                                    window.set_title("nai · 同源流式音画验收：正在播放");
                                }
                                let conversion_began = Instant::now();
                                copy_bgra_to_window(&mut screen, &frame.pixels);
                                let elapsed = conversion_began.elapsed().as_micros();
                                pixel_conversion_us += elapsed;
                                max_pixel_conversion_us = max_pixel_conversion_us.max(elapsed);
                                selected = frame.metadata.sequence;
                                selected_count += 1;
                                pixels_ready = true;
                            }
                            if clock.played_sample.is_some_and(|played| played > 0)
                                && let (Some(sender), Some(active)) =
                                    (&frame_observer, &observer_active)
                                && active.load(Ordering::Acquire)
                            {
                                let origin = observed_origin.take();
                                let after_origin = origin
                                    .as_ref()
                                    .and_then(|frame| frame.metadata().sequence)
                                    .or(observed_sequence);
                                for snapshot in origin
                                    .into_iter()
                                    .chain(s.copy_selected_verified(after_origin))
                                {
                                    let sequence = snapshot.metadata().sequence;
                                    match sender.try_send(SelectedFrame {
                                        frame: snapshot,
                                        clock: *clock,
                                        operation: operation.clone(),
                                        active: active.clone(),
                                        expires: event.deadline(),
                                    }) {
                                        Ok(()) => observed_sequence = sequence,
                                        Err(mpsc::TrySendError::Full(_)) => break,
                                        Err(mpsc::TrySendError::Disconnected(_)) => {
                                            active.store(false, Ordering::Release);
                                            break;
                                        }
                                    }
                                }
                            }
                        } else {
                            standby_paints +=
                                u64::from(crate::v561_standby::fill(&mut screen, standby));
                            pixels_ready = false;
                        }
                    }
                    latest = Some(event);
                }
                if !stopped && latest.as_ref().is_some_and(|event| !event.fresh()) {
                    return Err("device clock expired".into());
                }
                if stop_ms > 0
                    && running
                        .is_some_and(|at: Instant| at.elapsed() >= Duration::from_millis(stop_ms))
                {
                    requested_stop = true;
                    operation.cancel();
                    break;
                }
                operation
                    .check()
                    .map_err(|_| "cancelled before window submit")?;
                let submit_began = Instant::now();
                window.update_with_buffer(&screen, width as usize, height as usize)?;
                let elapsed = submit_began.elapsed().as_micros();
                pixel_witness.observe(
                    &screen,
                    (width as usize, height as usize),
                    window.get_size(),
                    window.is_active(),
                    if pixels_ready {
                        crate::v561_window_witness::Phase::Frame
                    } else {
                        crate::v561_window_witness::Phase::Standby
                    },
                );
                crate::v561_presentation::require_render_area(window.get_size(), &operation)?;
                window_submit_us += elapsed;
                max_window_submit_us = max_window_submit_us.max(elapsed);
                window_submissions += 1;
                if pixels_ready && last_submitted != selected {
                    if shown == 0 && diagnostic {
                        use std::io::Write;
                        let mut file = std::fs::File::create_new(dir.join("first-submitted.ppm"))?;
                        write!(file, "P6\n{width} {height}\n255\n")?;
                        let rgb = screen
                            .iter()
                            .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
                            .collect::<Vec<_>>();
                        file.write_all(&rgb)?;
                    }
                    shown += 1;
                    last_submitted = selected;
                }
                if result.is_none()
                    && let Ok(value) = done_rx.try_recv()
                {
                    result = Some(value.map_err(|e| e.as_str_name())?);
                }
                if audio_thread.is_finished() && gpu_thread.is_finished() {
                    break;
                }
            }
            Ok(())
        })();
        if loop_result.is_err() || requested_stop {
            operation.cancel();
        }
        if loop_result.is_err() || requested_stop || operation.check().is_err() {
            screen.fill(0);
            let _ = window.update_with_buffer(&screen, width as usize, height as usize);
        }
        let audio = audio_thread.join();
        if audio.is_err() {
            operation.cancel();
        }
        let returned = gpu_thread.join();
        let safe_standby = loop_result.is_ok()
            && !requested_stop
            && operation.check().is_ok()
            && audio.as_ref().is_ok_and(|r| r.is_ok())
            && returned.is_ok();
        if safe_standby {
            standby_paints += u64::from(crate::v561_standby::fill(&mut screen, standby));
        } else {
            screen.fill(0);
        }
        let submitted = window.update_with_buffer(&screen, width as usize, height as usize);
        if submitted.is_ok() {
            pixel_witness.observe(
                &screen,
                (width as usize, height as usize),
                window.get_size(),
                window.is_active(),
                if safe_standby {
                    crate::v561_window_witness::Phase::Standby
                } else {
                    crate::v561_window_witness::Phase::SafeBlack
                },
            );
        }
        if safe_standby && standby.is_some() {
            submitted?;
        }
        let returned = returned.map_err(|_| "gpu panic")?;
        if let Some(returned) = returned.filter(Resident::available) {
            self.owner = Some(returned);
        }
        let audio = audio.map_err(|_| "audio panic")?;
        // Preserve independent failures before cancellation masks the originating
        // branch. Only internal codes are recorded, never provider text or PCM.
        let loop_code = loop_result
            .as_ref()
            .err()
            .map(|error| match error.to_string().as_str() {
                "task cancelled or expired" => "CANCELLED_OR_EXPIRED",
                "stale device clock" | "device clock expired" => "CLOCK_EXPIRED",
                "device clock disconnected" => "CLOCK_DISCONNECTED",
                "clock rejected" => "CLOCK_REJECTED",
                "scheduler poisoned" => "SCHEDULER_POISONED",
                "cancelled before window submit" => "CANCELLED_BEFORE_SUBMIT",
                "OUTPUT_WINDOW_UNAVAILABLE" => "OUTPUT_WINDOW_UNAVAILABLE",
                value if value.starts_with("WORKBENCH_ERROR_") => "MEDIA_OPERATION_FAILED",
                _ => "WINDOW_OR_CONTROL_FAILED",
            });
        if loop_code.is_some() || audio.is_err() {
            serde_json::to_writer_pretty(
                std::fs::File::create_new(dir.join("failure.json"))?,
                &serde_json::json!({"loop_code":loop_code,
                    "audio_code":audio.as_ref().err().map(|e|e.as_str_name()),
                    "shown_frames":shown,"epoch":epoch,
                    "standby_visible":false,
                    "generation_progress":cached_observer.as_ref().map(|o|o.metrics())}),
            )?;
        }
        while let Ok(event) = clock_rx.try_recv() {
            let clock = event.observation();
            if clock.epoch == epoch
                && clock.device_generation == generation
                && clock.state == ClockState::Stopped as i32
            {
                stopped = true;
            }
        }
        loop_result?;
        if safe_standby {
            crate::v561_presentation::require_render_area(window.get_size(), &operation)?;
        }
        let report = if requested_stop {
            serde_json::json!({"scope":"local_device_stream_av","status":"stopped","shown_frames":shown,"audio_completed":audio.is_ok(),"model_api_calls":if prepared_asset{0}else{1}})
        } else {
            let audio = audio.map_err(|e| e.as_str_name())?;
            let (production, frames) = result
                .or_else(|| done_rx.try_recv().ok().and_then(Result::ok))
                .ok_or("GPU result missing")?;
            if audio.played_samples != production.samples || shown == 0 {
                return Err("device AV incomplete".into());
            }
            serde_json::json!({"scope":if prepared_asset{"prepared_asset_device_playback_not_live_generation"}else{"local_device_stream_av_not_obs_or_continuous_realtime"},"status":"completed","model_api_calls":if prepared_asset{0}else{1},
            "samples":production.samples,"played_samples":audio.played_samples,"frames":frames,"shown_frames":shown,"clock_selected_frames":selected_count,
            "device_process_id":audio.device_process_id,
            "prefill_samples":prefill,"first_pcm_ms":production.first_pcm_ms,"first_verified_frame_ms":production.first_frame_ms,
            "device_first_played_ms":audio.first_played_ms,"device_complete_ms":audio.completed_ms,"device_clock_observations":audio.clock_observations,
            "audio_first_payload_ms":audio.first_pcm_ms,"audio_startup_marks":audio.startup_marks,
                "max_clock_age_ms":audio.max_clock_age_ms,"device_generation":generation,"stopped_clock_seen":stopped,
                "device_completed_script_ms":timeline.map(|origin|audio.completed_at.saturating_duration_since(origin).as_millis()),
                "device_first_played_script_ms":timeline.map(|origin|audio.first_played_at.saturating_duration_since(origin).as_millis()),
                "source_adapter_first_pcm_ms":production.adapter_first_pcm_ms,
                "play_entry_first_sample_ms":audio.first_played_at.saturating_duration_since(play_entry).as_millis(),
                "play_entry_completed_ms":audio.completed_at.saturating_duration_since(play_entry).as_millis(),"prepared_asset":prepared_asset})
        };
        serde_json::to_writer_pretty(std::fs::File::create_new(dir.join("report.json"))?, &report)?;
        let mut report = report;
        report["output_window_identity"] = format!("{:p}", window.get_window_handle()).into();
        report["standby_paints"] = standby_paints.into();
        report["standby_visible_after_play"] = serde_json::json!(
            safe_standby
                && standby
                    .and_then(crate::v561_standby::View::pixels)
                    .is_some()
        );
        report["playback_profile"] = serde_json::json!({
            "scope":"host_aggregate_not_hardware_present_or_gpu_time",
            "device_prepare_wait_us":device_prepare_wait_us,
            "pixel_conversion_us":pixel_conversion_us,
            "max_pixel_conversion_us":max_pixel_conversion_us,
            "pixel_conversions":selected_count,
            "window_submit_us":window_submit_us,
            "max_window_submit_us":max_window_submit_us,
            "window_submissions":window_submissions
        });
        if let Some(manifest) = asset_manifest {
            report["asset_identity"] = manifest["identity_sha256"].clone();
            report["pcm_sha256"] = manifest["pcm_sha256"].clone();
        }
        report["video_source_mode"] = if prepared_asset {
            "prepared_asset"
        } else if cached_observer.is_some() {
            "incremental_gpu_stream"
        } else {
            "streaming"
        }
        .into();
        if let Some(observer) = cached_observer {
            let metrics = observer.metrics();
            report["generation_progress"] = metrics.clone();
            report["gpu_prefetch_started_script_ms"] = metrics["started_ms"].clone();
            report["gpu_first_frame_script_ms"] = metrics["first_frame_ms"].clone();
            report["gpu_first_consumed_frame_script_ms"] = metrics["first_consumed_ms"].clone();
            report["gpu_completed_script_ms"] = metrics["completed_ms"].clone();
            report["source_ready_script_ms"] = metrics["source_verified_ms"].clone();
        }
        report["task"] = epoch.into();
        report["epoch"] = epoch.into();
        report["avatar_available_after"] = self.available().into();
        report["worker_pid"] = self.worker_pid.into();
        serde_json::to_writer_pretty(
            std::fs::File::create_new(dir.join("session.json"))?,
            &report,
        )?;
        if stopped
            && !requested_stop
            && operation.check().is_ok()
            && observer_active
                .as_ref()
                .is_some_and(|active| active.load(Ordering::Acquire))
            && let (Some(completed), Some(samples)) =
                (observer_completed, report["samples"].as_u64())
        {
            let _ = completed.try_send(samples);
        }
        // The optional diagnostic preview hold must never delay encoder EOF.
        drop(frame_observer);
        if diagnostic {
            window.set_title("nai · 播放已结束：静音安全画面（15秒后关闭，可直接关闭）");
            let until = Instant::now() + Duration::from_secs(15);
            while window.is_open() && Instant::now() < until && !window.is_key_down(Key::Escape) {
                window.update_with_buffer(&screen, width as usize, height as usize)?;
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod playback_profile_tests {
    use super::*;

    #[test]
    fn window_pixels_preserve_bgr_channels_and_clear_alpha() {
        let mut screen = [0xffffffff; 2];
        copy_bgra_to_window(&mut screen, &[1, 2, 3, 255, 254, 128, 64, 17]);
        assert_eq!(screen, [0x00030201, 0x004080fe]);
    }

    #[test]
    #[ignore = "explicit CPU profiling; timings are diagnostic, not acceptance"]
    fn profile_window_pixel_conversion() {
        let pixels = vec![137u8; 768 * 768 * 4];
        let mut screen = vec![0u32; 768 * 768];
        let mut times = Vec::with_capacity(120);
        for _ in 0..120 {
            let began = Instant::now();
            copy_bgra_to_window(
                std::hint::black_box(&mut screen),
                std::hint::black_box(&pixels),
            );
            times.push(began.elapsed().as_nanos());
            std::hint::black_box(&screen);
        }
        times.sort_unstable();
        println!(
            "pixel_conversion_768_ns p50={} p95={} max={}",
            times[60], times[114], times[119]
        );
        assert!(screen.iter().all(|pixel| *pixel == 0x00898989));
    }
    #[test]
    fn pcm_observer_preserves_offsets_and_drops_cancelled_blocks() {
        let operation = Operation::model_task(1000).unwrap();
        let (sender, receiver) = mpsc::sync_channel(2);
        let mut observer = PcmObserver {
            sender,
            operation: operation.clone(),
            next_sample: 0,
            active: Arc::new(AtomicBool::new(true)),
        };
        assert!(observer.publish(7, 4, &[1, 2, 3, 4]));
        assert!(observer.publish(7, 4, &[5, 6]));
        let (epoch, generation, offset, bytes) = receiver.try_recv().unwrap().into_parts().unwrap();
        assert_eq!(
            (epoch, generation, offset, bytes.as_slice()),
            (7, 4, 0, &[1, 2, 3, 4][..])
        );
        let second = receiver.try_recv().unwrap();
        assert_eq!(second.start_sample, 2);
        assert!(observer.publish(7, 4, &[7, 8]));
        assert!(observer.publish(7, 4, &[9, 10]));
        assert!(!observer.publish(7, 4, &[11, 12]));
        assert!(!observer.active.load(Ordering::Acquire));
        assert!(receiver.try_recv().unwrap().into_parts().is_none());
        operation.cancel();
        assert!(second.into_parts().is_none());
    }
}
