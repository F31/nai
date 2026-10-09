//! Explicit `--render-drill` producer. It renders real 2D pixels with the
//! `nai-avatar-2d` plugin and writes them into a root-reserved frame slot through
//! an inherited pool FD (delivered over stderr). Events flow over stdout as
//! bounded Protobuf frames; PCM is raw bytes and never crosses a JSON/Base64
//! control bus.
//!
//! Boundary: this produces no `PlaybackAck`, declares no Ready and grants no
//! output authorization or device access. Windows and non-Linux backends are
//! `CapabilityNotReady`.
use nai_avatar_2d::{AssetBytes, Avatar, LicenseGrants};
use nai_contracts::{
    candidate_hash,
    v1::{
        ContractError, ErrorCode, FrameSlotEvent, FrameSlotTicket, PerformanceCue,
        frame_slot_event::Event,
    },
};
use nai_media_core::{
    frame_slots::{
        IoBudget,
        shared_ram::{validate_producer_handle, write_frame},
        validate_ticket,
    },
    timeline::audio::{AudioBlock, SampleFormat},
};
use nai_plugin_sdk::{frame, plugin::PluginContext};
use prost::Message;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const USAGE: &str = "Usage: nai-render-worker --render-drill --pcm <path> --ticket <path> --epoch <u64> --avatar <manifest.json> --avatar-config <config.json> --asset <id=path>... --license <ref>... [--ready <path>]; pool FD is inherited via stderr";

/// The root reserves the slot with a non-renewable 250ms budget, so the worker
/// admits the avatar (a one-time ~200ms schema cost) and signals readiness before
/// the root opens that window; it then waits a bounded time for the reserved
/// ticket to appear.
const TICKET_WAIT: Duration = Duration::from_secs(10);
const ADMIT_BUDGET: Duration = Duration::from_secs(10);

fn failure(code: ErrorCode, message: &str) -> ContractError {
    ContractError {
        category: "render-worker".into(),
        code: code as i32,
        message: message.into(),
        ..Default::default()
    }
}

fn utc_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

/// Parsed invocation. Paths are root-supplied; the worker opens only these and
/// the inherited pool FD.
#[derive(Debug, Clone)]
pub struct Config {
    pub pcm: PathBuf,
    pub ticket: PathBuf,
    pub epoch: u64,
    pub manifest: PathBuf,
    pub config: PathBuf,
    pub assets: Vec<(String, PathBuf)>,
    pub licenses: Vec<String>,
    /// Drill warm-up marker written once the avatar is admitted and the worker is
    /// waiting for the root-reserved ticket. The root uses it to open the slot
    /// reserve window only after the one-time admit cost is paid.
    pub ready: Option<PathBuf>,
    pub broker: Option<PathBuf>,
    pub finish: Option<PathBuf>,
}

impl Config {
    pub fn usage() -> &'static str {
        USAGE
    }

    pub fn parse(args: &[String]) -> Result<Self, String> {
        if args.first().map(String::as_str) != Some("--render-drill") {
            return Err(USAGE.into());
        }
        let mut pcm = None;
        let mut ticket = None;
        let mut epoch = None;
        let mut manifest = None;
        let mut config = None;
        let mut assets = Vec::new();
        let mut licenses = Vec::new();
        let mut ready = None;
        let mut broker = None;
        let mut finish = None;
        let mut index = 1;
        while index < args.len() {
            let flag = args[index].clone();
            let value = args
                .get(index + 1)
                .cloned()
                .ok_or_else(|| format!("missing value for {flag}"))?;
            match flag.as_str() {
                "--pcm" => pcm = Some(PathBuf::from(value)),
                "--ticket" => ticket = Some(PathBuf::from(value)),
                "--epoch" => {
                    epoch = Some(
                        value
                            .parse::<u64>()
                            .map_err(|_| "invalid --epoch".to_string())?,
                    )
                }
                "--avatar" => manifest = Some(PathBuf::from(value)),
                "--avatar-config" => config = Some(PathBuf::from(value)),
                "--ready" => ready = Some(PathBuf::from(value)),
                "--broker" => broker = Some(PathBuf::from(value)),
                "--finish" => finish = Some(PathBuf::from(value)),
                "--asset" => {
                    let (id, path) = value
                        .split_once('=')
                        .ok_or_else(|| "--asset requires id=path".to_string())?;
                    if id.is_empty() || path.is_empty() {
                        return Err("--asset requires id=path".into());
                    }
                    assets.push((id.to_string(), PathBuf::from(path)));
                }
                "--license" => {
                    if value.is_empty() {
                        return Err("--license requires a reference".into());
                    }
                    licenses.push(value);
                }
                other => return Err(format!("unknown argument {other}")),
            }
            index += 2;
        }
        if assets.len() != 3 {
            return Err("exactly three --asset id=path inputs are required".into());
        }
        if licenses.is_empty() {
            return Err("at least one --license reference is required".into());
        }
        if broker.is_some() && (ready.is_none() || finish.is_none()) {
            return Err("broker drill requires --ready and --finish".into());
        }
        Ok(Self {
            pcm: pcm.ok_or_else(|| "--pcm is required".to_string())?,
            ticket: ticket.ok_or_else(|| "--ticket is required".to_string())?,
            epoch: epoch.ok_or_else(|| "--epoch is required".to_string())?,
            manifest: manifest.ok_or_else(|| "--avatar is required".to_string())?,
            config: config.ok_or_else(|| "--avatar-config is required".to_string())?,
            assets,
            licenses,
            ready,
            broker,
            finish,
        })
    }
}

fn read_json(path: &PathBuf) -> Result<Value, ContractError> {
    let bytes = fs::read(path).map_err(|_| {
        failure(
            ErrorCode::CapabilityNotReady,
            "avatar input file unreadable",
        )
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|_| failure(ErrorCode::InvalidArgument, "avatar JSON rejected"))
}

fn write_event(out: &mut impl Write, event: Event) -> Result<(), ContractError> {
    let message = FrameSlotEvent { event: Some(event) };
    frame::write(out, &message.encode_to_vec())?;
    out.flush()
        .map_err(|_| failure(ErrorCode::Internal, "event write failed"))
}

/// Renders one real frame and writes it into the reserved slot. On any failure it
/// emits an `error` event and returns the error (the process exits non-zero).
pub fn run(config: &Config, pool: &File, out: &mut impl Write) -> Result<(), ContractError> {
    match produce(config, pool, out) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = write_event(out, Event::Error(error.clone()));
            Err(error)
        }
    }
}

fn produce(config: &Config, pool: &File, out: &mut impl Write) -> Result<(), ContractError> {
    let manifest = read_json(&config.manifest)?;
    let avatar_config = read_json(&config.config)?;
    let asset_bytes = config
        .assets
        .iter()
        .map(|(_, path)| {
            fs::read(path)
                .map_err(|_| failure(ErrorCode::CapabilityNotReady, "asset file unreadable"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let assets: Vec<AssetBytes<'_>> = config
        .assets
        .iter()
        .zip(&asset_bytes)
        .map(|((id, _), bytes)| AssetBytes { id, bytes })
        .collect();
    let grants = LicenseGrants::from(config.licenses.iter().cloned().collect::<BTreeSet<_>>());

    let pcm_bytes = fs::read(&config.pcm)
        .map_err(|_| failure(ErrorCode::CapabilityNotReady, "PCM file unreadable"))?;
    if pcm_bytes.is_empty() || !pcm_bytes.len().is_multiple_of(4) || pcm_bytes.len() > 1 << 22 {
        return Err(failure(
            ErrorCode::InvalidArgument,
            "PCM must be bounded raw little-endian f32 bytes",
        ));
    }
    let samples: Vec<f32> = pcm_bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    let sample_rate = avatar_config["sample_rate"]
        .as_u64()
        .filter(|rate| (1..=u32::MAX as u64).contains(rate))
        .ok_or_else(|| failure(ErrorCode::InvalidArgument, "avatar sample rate rejected"))?
        as u32;

    // Admit the concrete avatar before the root opens the non-renewable slot
    // reserve window: the one-time schema/runtime cost (measured ~200ms) must not
    // spend the ticket's 250ms budget. The admit context carries no session scope;
    // the render context below is rebuilt from the reserved ticket.
    let admit_context = PluginContext::new(
        "nai-avatar-2d",
        "",
        1,
        ADMIT_BUDGET.as_millis() as u32,
        json!({}),
        Vec::new(),
        Instant::now() + ADMIT_BUDGET,
        Arc::new(AtomicBool::new(false)),
        utc_ms(),
    );
    let avatar = Arc::new(Avatar::admit(
        &admit_context,
        &manifest,
        &avatar_config,
        &assets,
        &grants,
    )?);
    if let Some(path) = &config.broker {
        nai_media_core::frame_slots::broker::connect_trigger(path)?;
    }
    if let Some(path) = &config.ready {
        fs::write(path, b"ready")
            .map_err(|_| failure(ErrorCode::CapabilityNotReady, "ready marker unwritable"))?;
    }

    let ticket_bytes = await_ticket(&config.ticket)?;
    let ticket = FrameSlotTicket::decode(ticket_bytes.as_slice())
        .map_err(|_| failure(ErrorCode::InvalidArgument, "ticket protobuf rejected"))?;
    let bytes = validate_ticket(&ticket)?;
    if ticket.generation_epoch != config.epoch {
        return Err(failure(
            ErrorCode::StaleEpoch,
            "ticket epoch does not match the current session epoch",
        ));
    }
    let reserved = ticket
        .frame
        .clone()
        .ok_or_else(|| failure(ErrorCode::InvalidArgument, "ticket has no frame metadata"))?;

    let budget_ms = ticket.remaining_budget_ms;
    let now_utc = utc_ms();
    let context = PluginContext::new(
        "nai-avatar-2d",
        &ticket.session_id,
        ticket.generation_epoch,
        budget_ms,
        json!({}),
        Vec::new(),
        Instant::now() + Duration::from_millis(u64::from(budget_ms)),
        Arc::new(AtomicBool::new(false)),
        now_utc,
    );
    context.check()?;

    let mut cue = PerformanceCue {
        session_id: ticket.session_id.clone(),
        utterance_id: reserved.utterance_id.clone(),
        generation_epoch: ticket.generation_epoch,
        speech_text: "render-drill".into(),
        expires_at_utc_ms: now_utc + u64::from(budget_ms).max(1),
        hash_algorithm: "sha256-canonical-v1".into(),
        ..Default::default()
    };
    cue.candidate_hash = candidate_hash(&cue);
    let mut stream = avatar.stream(&context, &cue)?;

    let block = AudioBlock {
        session_id: ticket.session_id.clone(),
        utterance_id: reserved.utterance_id.clone(),
        generation_epoch: ticket.generation_epoch,
        seq: 0,
        sample_rate,
        channels: 1,
        format: SampleFormat::F32,
        sample_start: 0,
        sample_count: samples.len() as u32,
        end_of_stream: true,
    };
    stream.push(&context, &grants, block, &samples)?;
    let rendered = stream
        .next_frame(
            &context,
            &grants,
            &format!("render-drill:{}", ticket.slot_index),
        )?
        .ok_or_else(|| {
            failure(
                ErrorCode::ResourceExhausted,
                "PCM did not fill exactly one frame",
            )
        })?;
    if rendered.metadata.width != reserved.width
        || rendered.metadata.height != reserved.height
        || rendered.metadata.duration != reserved.duration
        || rendered.metadata.generation_epoch != ticket.generation_epoch
        || rendered.rgba.len() as u32 != bytes
    {
        return Err(failure(
            ErrorCode::InvalidArgument,
            "rendered frame does not match the reserved ticket",
        ));
    }

    // Recheck epoch, cancellation and deadline immediately before crossing the
    // write boundary; stale or over-budget data must never enter the slot.
    context.check()?;
    if utc_ms() >= cue.expires_at_utc_ms {
        return Err(failure(
            ErrorCode::DeadlineExceeded,
            "render budget expired before the write",
        ));
    }

    write_event(out, Event::WriteStarted(true))?;
    let budget = IoBudget::new(Duration::from_millis(u64::from(ticket.remaining_budget_ms)))?;
    validate_producer_handle(pool, &ticket)?;
    let commit = write_frame(pool, &ticket, &rendered.rgba, &budget)?;
    write_event(out, Event::Committed(Box::new(commit)))?;
    if let Some(path) = &config.finish {
        let deadline = Instant::now() + TICKET_WAIT;
        while !path.is_file() {
            if Instant::now() >= deadline {
                return Err(failure(
                    ErrorCode::DeadlineExceeded,
                    "root did not finish frame transaction",
                ));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    Ok(())
}

/// Waits a bounded time for the root to deliver the reserved ticket. The file is
/// expected to appear atomically (rename), so a present non-empty file is whole.
fn await_ticket(path: &Path) -> Result<Vec<u8>, ContractError> {
    let deadline = Instant::now() + TICKET_WAIT;
    loop {
        if let Ok(bytes) = fs::read(path)
            && !bytes.is_empty()
        {
            return Ok(bytes);
        }
        if Instant::now() >= deadline {
            return Err(failure(
                ErrorCode::DeadlineExceeded,
                "root did not deliver the reserved ticket",
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }
    fn valid() -> Vec<String> {
        args(&[
            "--render-drill",
            "--pcm",
            "pcm.bin",
            "--ticket",
            "ticket.bin",
            "--epoch",
            "7",
            "--avatar",
            "manifest.json",
            "--avatar-config",
            "config.json",
            "--asset",
            "base=base.rgba",
            "--asset",
            "closed=closed.rgba",
            "--asset",
            "open=open.rgba",
            "--license",
            "test-license",
        ])
    }

    #[test]
    fn parsing_requires_explicit_render_drill_and_exact_asset_closure() {
        assert!(Config::parse(&valid()).is_ok());
        assert!(Config::parse(&args(&["--pcm", "x"])).is_err());
        let mut missing_asset = valid();
        missing_asset.truncate(missing_asset.len() - 4);
        assert!(Config::parse(&missing_asset).is_err());
        let mut no_license = valid();
        no_license.truncate(no_license.len() - 2);
        assert!(Config::parse(&no_license).is_err());
        let mut unknown = valid();
        unknown.push("--nope".into());
        unknown.push("x".into());
        assert!(Config::parse(&unknown).is_err());
    }
}
