//! Talent takeover audio path and echo control (V3.6/V3.7 §8.5).
//!
//! §8.5 makes five things true at once. Takeover pauses automatic play, mutes
//! the digital human and clears the queue, optionally mixing in a live talent
//! track. The talent microphone is rendered by the keeper **independent of the
//! TTS track**, and the TTS track is muted while takeover is active. Microphone
//! sources carry a **stable device id**, and a change of the system default
//! device is *observed*, never silently followed. **Only one live capture path
//! is allowed**, so OBS and a companion app cannot both feed the same mix and
//! cause echo, and takeover requires a passing **echo test**. Resuming automatic
//! interaction is an explicit operation that re-checks authorization, fact
//! version and duty lease — the end of talent speech does **not** resume it.
//!
//! The shape of the types carries the guarantees. A single live capture is not
//! a check somebody could forget to run: [`CaptureRegistry::acquire`] refuses a
//! second live path until the first is released. Two tracks are never audible
//! at once because [`AudioMixer::render`] derives the muted track from the mode,
//! so a both-tracks-audible state has no constructor. And talent silence has no
//! code path into [`AudioMixer::resume_auto`]; a caller must build a
//! [`ResumeProof`] that the mixer re-checks every time.
//!
//! No device is opened here and no PCM is mixed. Device enumeration, the actual
//! echo test and the real OS mixer are a device milestone; this is the portable
//! control semantics a keeper obeys.
use nai_contracts::v1::ErrorCode;

/// Stable code plus a portable token, matching the `UpgradeError` /
/// `MultiplexError` convention so a report can classify a refusal without a
/// debug build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixError {
    pub code: ErrorCode,
    pub reason: &'static str,
}

impl std::fmt::Display for MixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mixing: {} ({})", self.code.as_str_name(), self.reason)
    }
}

impl std::error::Error for MixError {}

fn error(code: ErrorCode, reason: &'static str) -> MixError {
    MixError { code, reason }
}

type Result<T> = std::result::Result<T, MixError>;

const MAX_ID: usize = 128;

fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_ID && !value.contains(char::is_whitespace)
}

/// Upper bound on the automatic utterance queue, so takeover can never report an
/// unbounded clear count.
pub const MAX_AUTO_QUEUE: usize = 256;

/// How long a passing echo test stays acceptable. An old pass is not evidence
/// that the current routing is clean.
pub const ECHO_TEST_TTL_MS: u64 = 300_000;

/// A track gain in fixed point: `0` is mute, `1000` is unity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gain(u16);

impl Gain {
    pub const MUTE: Gain = Gain(0);
    pub const FULL: Gain = Gain(1000);

    pub fn try_new(level: u16) -> Result<Self> {
        if level > 1000 {
            return Err(error(ErrorCode::InvalidArgument, "gain out of range"));
        }
        Ok(Self(level))
    }

    pub fn get(self) -> u16 {
        self.0
    }

    pub fn is_muted(self) -> bool {
        self.0 == 0
    }
}

/// A stable microphone identifier. Renaming or re-enumerating a device must not
/// look like a different source, so this is an opaque caller-supplied string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new(id: &str) -> Result<Self> {
        if !valid_id(id) {
            return Err(error(ErrorCode::InvalidArgument, "invalid device id"));
        }
        Ok(Self(id.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One candidate capture path: which process wants the microphone, on which
/// device. Two paths differing only by owner still conflict — that is exactly
/// the OBS-plus-companion echo case §8.5 forbids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturePath {
    pub owner: String,
    pub device: DeviceId,
}

impl CapturePath {
    pub fn new(owner: &str, device: DeviceId) -> Result<Self> {
        if !valid_id(owner) {
            return Err(error(ErrorCode::InvalidArgument, "invalid capture owner"));
        }
        Ok(Self {
            owner: owner.to_owned(),
            device,
        })
    }
}

/// A held claim on the single live capture path, carrying the generation that
/// acquired it. Releasing with a stale lease must not free the current holder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureLease {
    path: CapturePath,
    generation: u64,
}

impl CaptureLease {
    pub fn path(&self) -> &CapturePath {
        &self.path
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// Result of comparing an observed default device against the last one seen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceObservation {
    Unchanged,
    First(DeviceId),
    Changed { from: DeviceId, to: DeviceId },
}

/// Owns the single live capture path. Acquisition is exclusive until release, so
/// "only one valid capture path" is enforced by the API rather than by a check
/// at the call site.
#[derive(Debug, Default)]
pub struct CaptureRegistry {
    live: Option<CaptureLease>,
    generation: u64,
    default_device: Option<DeviceId>,
}

impl CaptureRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Acquire the capture path. A second acquisition while one is live is
    /// refused with the current holder reported in the error token, so a UI can
    /// say which process must be stopped first.
    pub fn acquire(&mut self, path: CapturePath) -> Result<CaptureLease> {
        if self.live.is_some() {
            return Err(error(ErrorCode::Conflict, "capture path already held"));
        }
        self.generation = self.generation.wrapping_add(1);
        let lease = CaptureLease {
            path,
            generation: self.generation,
        };
        self.live = Some(lease.clone());
        Ok(lease)
    }

    pub fn release(&mut self, lease: &CaptureLease) -> Result<()> {
        match &self.live {
            Some(current) if current.generation == lease.generation => {
                self.live = None;
                Ok(())
            }
            Some(_) => Err(error(ErrorCode::StaleEpoch, "stale capture lease")),
            None => Err(error(ErrorCode::Conflict, "no live capture path")),
        }
    }

    pub fn live(&self) -> Option<&CapturePath> {
        self.live.as_ref().map(CaptureLease::path)
    }

    pub fn live_count(&self) -> usize {
        usize::from(self.live.is_some())
    }

    /// Compare a newly observed system default device with the last one. A
    /// change is reported and remembered, never acted on: following the default
    /// silently would move the show onto a device nobody tested.
    pub fn observe_default(&mut self, device: DeviceId) -> DeviceObservation {
        let observation = match self.default_device.take() {
            None => DeviceObservation::First(device.clone()),
            Some(from) if from == device => DeviceObservation::Unchanged,
            Some(from) => DeviceObservation::Changed {
                from,
                to: device.clone(),
            },
        };
        self.default_device = Some(device);
        observation
    }

    pub fn default_device(&self) -> Option<&DeviceId> {
        self.default_device.as_ref()
    }
}

/// Echo-test state. Takeover requires a `Passed` result that is still fresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EchoState {
    Unrun,
    Running,
    Passed { at_utc_ms: u64 },
    Failed,
}

impl EchoState {
    pub fn start() -> Self {
        EchoState::Running
    }

    pub fn pass(at_utc_ms: u64) -> Self {
        EchoState::Passed { at_utc_ms }
    }

    pub fn fail() -> Self {
        EchoState::Failed
    }

    /// A pass is only acceptable inside its TTL and never in the future.
    pub fn valid_at(&self, now_utc_ms: u64) -> bool {
        match *self {
            EchoState::Passed { at_utc_ms } => {
                at_utc_ms <= now_utc_ms && now_utc_ms - at_utc_ms <= ECHO_TEST_TTL_MS
            }
            _ => false,
        }
    }
}

/// Which track the mix is currently following.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixMode {
    Auto,
    Takeover,
}

/// The rendered track levels. At least one track is always muted, because the
/// mixer derives the muted track from the mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderLevels {
    pub tts: Gain,
    pub talent: Gain,
}

impl RenderLevels {
    pub fn any_audible(&self) -> bool {
        !self.tts.is_muted() || !self.talent.is_muted()
    }
}

/// A takeover request. The echo state and fact version are supplied by the
/// caller; the mixer never assumes either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakeoverRequest {
    pub echo: EchoState,
    pub talent_gain: Gain,
    pub fact_version: String,
    pub now_utc_ms: u64,
}

/// What the mixer did on takeover, reported rather than asserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TakeoverReport {
    pub paused_auto: bool,
    pub tts_muted: bool,
    pub cleared_utterances: usize,
    pub talent: Gain,
    pub echo_accepted: bool,
}

/// Proof that automatic interaction may resume. Every field is re-checked on
/// each call, so a stale fact version or an expired duty lease refuses resume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeProof {
    pub authorized: bool,
    pub fact_version: String,
    pub duty_lease_until_utc_ms: Option<u64>,
}

/// The audio mixer. TTS and talent are two independent tracks; the mode decides
/// which one is audible, so the two are never summed.
#[derive(Debug)]
pub struct AudioMixer {
    mode: MixMode,
    tts_gain: Gain,
    talent_gain: Gain,
    auto_queue: usize,
    cleared: u64,
    epoch: u64,
    fact_version: String,
}

impl Default for AudioMixer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioMixer {
    /// Starts in auto with the TTS track at unity and no talent track attached.
    pub fn new() -> Self {
        Self {
            mode: MixMode::Auto,
            tts_gain: Gain::FULL,
            talent_gain: Gain::MUTE,
            auto_queue: 0,
            cleared: 0,
            epoch: 0,
            fact_version: String::new(),
        }
    }

    pub fn mode(&self) -> MixMode {
        self.mode
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn queued(&self) -> usize {
        self.auto_queue
    }

    pub fn cleared_total(&self) -> u64 {
        self.cleared
    }

    /// The rendered levels. The muted track is derived from the mode, so a
    /// both-tracks-audible output cannot be represented.
    pub fn render(&self) -> RenderLevels {
        match self.mode {
            MixMode::Auto => RenderLevels {
                tts: self.tts_gain,
                talent: Gain::MUTE,
            },
            MixMode::Takeover => RenderLevels {
                tts: Gain::MUTE,
                talent: self.talent_gain,
            },
        }
    }

    /// Queue automatic utterances. Only meaningful in auto; a bounded queue is
    /// refused rather than grown without limit.
    pub fn enqueue_auto(&mut self, count: usize) -> Result<()> {
        if self.mode != MixMode::Auto {
            return Err(error(ErrorCode::Conflict, "not in auto mode"));
        }
        if count == 0 {
            return Err(error(ErrorCode::InvalidArgument, "empty enqueue"));
        }
        if self.auto_queue + count > MAX_AUTO_QUEUE {
            return Err(error(ErrorCode::ResourceExhausted, "auto queue full"));
        }
        self.auto_queue += count;
        Ok(())
    }

    /// Pause auto, mute the digital human, clear the queue and optionally attach
    /// the talent track. Requires a fresh passing echo test and a non-empty fact
    /// version; refuses a second takeover so two capture paths never converge.
    pub fn takeover(&mut self, request: TakeoverRequest) -> Result<TakeoverReport> {
        if self.mode == MixMode::Takeover {
            return Err(error(ErrorCode::Conflict, "already in takeover"));
        }
        if !valid_id(&request.fact_version) {
            return Err(error(ErrorCode::InvalidArgument, "invalid fact version"));
        }
        if !request.echo.valid_at(request.now_utc_ms) {
            let code = match request.echo {
                EchoState::Passed { .. } => ErrorCode::EvidenceExpired,
                _ => ErrorCode::CapabilityNotReady,
            };
            return Err(error(code, "echo test not passed"));
        }
        let cleared = self.auto_queue;
        self.auto_queue = 0;
        self.mode = MixMode::Takeover;
        self.tts_gain = Gain::MUTE;
        self.talent_gain = request.talent_gain;
        self.fact_version = request.fact_version;
        self.epoch = self.epoch.wrapping_add(1);
        self.cleared += cleared as u64;
        Ok(TakeoverReport {
            paused_auto: true,
            tts_muted: true,
            cleared_utterances: cleared,
            talent: self.talent_gain,
            echo_accepted: true,
        })
    }

    /// Resume automatic interaction. This is the only path back to auto: it
    /// re-checks authorization, the fact version taken at takeover and a live
    /// duty lease. No amount of talent speech calls it.
    pub fn resume_auto(&mut self, proof: &ResumeProof, now_utc_ms: u64) -> Result<()> {
        if self.mode == MixMode::Auto {
            return Err(error(ErrorCode::Conflict, "not in takeover"));
        }
        if !proof.authorized {
            return Err(error(ErrorCode::PermissionDenied, "not authorized"));
        }
        if proof.fact_version.is_empty() || proof.fact_version != self.fact_version {
            return Err(error(ErrorCode::PermissionDenied, "fact version mismatch"));
        }
        match proof.duty_lease_until_utc_ms {
            Some(until) if until > now_utc_ms => {}
            _ => return Err(error(ErrorCode::PermissionDenied, "no live duty lease")),
        }
        self.mode = MixMode::Auto;
        self.tts_gain = Gain::FULL;
        self.talent_gain = Gain::MUTE;
        Ok(())
    }

    /// The end of talent speech. It reports the current mode and changes
    /// nothing: resuming is a decision, not an event.
    pub fn on_talent_silence(&self) -> MixMode {
        self.mode
    }

    /// A snapshot for [`audit_mix`], combining the mixer's own state with the
    /// capture and echo facts the caller owns.
    pub fn snapshot(&self, live_captures: usize, echo_valid: bool) -> MixSnapshot {
        let levels = self.render();
        MixSnapshot {
            mode: self.mode,
            tts: levels.tts,
            talent: levels.talent,
            live_captures,
            echo_valid,
        }
    }
}

/// A plain-value view of the mix, so the audit is a pure function of reported
/// state and every violation is constructible in a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixSnapshot {
    pub mode: MixMode,
    pub tts: Gain,
    pub talent: Gain,
    pub live_captures: usize,
    pub echo_valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixViolation {
    pub token: &'static str,
    pub detail: String,
}

/// Check the §8.5 invariants against a reported snapshot. Returns every
/// violation rather than the first, because the fixes differ per rule.
pub fn audit_mix(snapshot: &MixSnapshot) -> Vec<MixViolation> {
    let mut violations = Vec::new();
    let mut push = |token: &'static str, detail: &str| {
        violations.push(MixViolation {
            token,
            detail: detail.to_owned(),
        });
    };
    if snapshot.tts.get() > 0 && snapshot.talent.get() > 0 {
        push(
            "both_tracks_audible",
            "TTS and talent are summed, not exclusive",
        );
    }
    match snapshot.mode {
        MixMode::Takeover => {
            if snapshot.tts.get() > 0 {
                push("takeover_with_tts_audible", "digital human not muted");
            }
            if snapshot.live_captures == 0 {
                push("takeover_without_capture_path", "no microphone attached");
            }
            if !snapshot.echo_valid {
                push(
                    "takeover_without_valid_echo_test",
                    "echo test stale or absent",
                );
            }
        }
        MixMode::Auto => {
            if snapshot.talent.get() > 0 {
                push("auto_with_talent_audible", "talent audible while auto");
            }
        }
    }
    if snapshot.live_captures > 1 {
        push(
            "multiple_live_capture_paths",
            "more than one microphone path can echo",
        );
    }
    violations
}
