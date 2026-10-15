//! The engineering engine assembly: single storage owner, shared session,
//! loopback control surface and supervised media children in one runnable
//! process.
//!
//! It is not the production form. There is no protected IPC, no device or window
//! binding; the media drill is explicit and runs on the Linux tmpfs backend, and
//! the manual-event rehearsal lane renders real frames into memory only. Neither
//! reports hardware output or a device PlaybackAck.
// The media drill is Linux-only. On other targets its helpers, constants and
// asset fields have no caller, which is a platform fact rather than dead code.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]
// Slot-commit types are only named by the Linux drill bodies; importing them
// unconditionally would leave the Windows build with unused imports.
use nai_contracts::v1::{FrameMetadata, FrameSlotTicket};
#[cfg(target_os = "linux")]
use nai_contracts::v1::{FrameSlotEvent, frame_slot_event::Event};
use nai_control_api::{Access, ControlApi, serve};
#[cfg(target_os = "linux")]
use nai_media_core::frame_slots::broker::PeerIdentity;
use nai_media_core::frame_slots::{
    PoolSpec,
    broker::{FrameBroker, PeerRole},
};
#[cfg(target_os = "linux")]
use nai_runtime::RenderWorkerSpec;
use nai_runtime::{
    BRINGUP_OPERATION, Binaries, Composition, ControlAction, FramePoolArgs, MemoryAdapter,
    Supervisor,
};
use nai_storage::{DataPaths, DecisionKind};
use prost::Message;
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{net::TcpListener, sync::watch};

pub mod pipeline;

pub const USAGE: &str = "Usage: nai-engine --data-root <dir> --workspace <id> --actor <id> --profile <name> \
--token-file <path> [--control-port <port>] [--halt-file <path>] \
[--keeper <path> --render-worker <path>] \
[--provision <runtime-profile.json>] \
[--media-drill --pcm <path> --avatar <manifest> --avatar-config <config> --asset <id=path> --license <id> \
[--slot-bytes <bytes>] [--time-base-den <n>]] \
[--event-file <script.json> --rehearsal-provider [--inject <none|knowledge|guard|generate|speech|storage>]]; \
real devices, production IPC and real provider transport unavailable";

/// Control-surface permissions of the local operator credential.
// ⚠️ `Access::new` 允许至多 **8** 条权限，本表已**正好用满**。再增一条必须先改那个上限。
const CONTROL_PERMISSIONS: &[&str] = &[
    "runtime.read",
    "session.read",
    "media.mute",
    "session.takeover",
    "approval.read",
    "approval.write",
    // §42.3/§42.4 记忆面。单独成权是为了可以**只**吊销记忆访问而不连带会话读取。
    "memory.read",
    "memory.forget",
];
const CONTROL_TTL: Duration = Duration::from_secs(3600);
const PRODUCER: &str = "render-1";
const CONSUMER: &str = "keeper-1";
const AVATAR_TIMEOUT: Duration = Duration::from_secs(15);
const DRILL_POLL: Duration = Duration::from_millis(5);

/// Explicit `--media-drill` configuration: real avatar assets, raw PCM and a
/// bounded pool. Nothing here is synthetic.
#[derive(Debug, Clone)]
pub struct MediaDrill {
    pub pcm: PathBuf,
    pub avatar_manifest: PathBuf,
    pub avatar_config: PathBuf,
    pub assets: Vec<(String, PathBuf)>,
    pub licenses: Vec<String>,
    pub slot_bytes: u32,
    pub time_base_den: u32,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub data_root: PathBuf,
    pub workspace: String,
    pub actor: String,
    pub profile: String,
    /// Trusted-owner provisioning: create the workspace, grant the runtime
    /// permissions, store this runtime profile, then exit. Never a model path.
    pub provision: Option<PathBuf>,
    pub token_file: PathBuf,
    pub control_port: u16,
    /// Explicit graceful-halt request path. `tokio::signal` needs
    /// `signal-hook-registry`, which the offline cache does not carry, and an
    /// `unsafe` handler is forbidden: the trusted owner stops the engine through
    /// this file instead. Killed processes are not a graceful halt.
    pub halt_file: Option<PathBuf>,
    pub keeper: Option<PathBuf>,
    pub render_worker: Option<PathBuf>,
    pub media_drill: Option<MediaDrill>,
    /// Explicit manual-event rehearsal script. Its presence selects the pipeline
    /// path; the engine never runs it implicitly.
    pub event_file: Option<PathBuf>,
    /// Rehearsal-only deterministic generator. The real provider transport is
    /// not wired for the manual-event pipeline, so this must be explicit.
    pub rehearsal_provider: bool,
    pub injection: pipeline::Injection,
}

#[derive(Default)]
struct DrillParts {
    enabled: bool,
    pcm: Option<PathBuf>,
    manifest: Option<PathBuf>,
    config: Option<PathBuf>,
    assets: Vec<(String, PathBuf)>,
    licenses: Vec<String>,
    slot_bytes: u32,
    time_base_den: u32,
}

/// Parses the explicit argument surface. An unknown or incomplete flag is an
/// error: the engine never substitutes a default for a missing input.
pub fn parse(args: &[String]) -> Result<Config, String> {
    let mut config = Config {
        data_root: PathBuf::new(),
        workspace: String::new(),
        actor: String::new(),
        profile: String::new(),
        provision: None,
        token_file: PathBuf::new(),
        control_port: 0,
        halt_file: None,
        keeper: None,
        render_worker: None,
        media_drill: None,
        event_file: None,
        rehearsal_provider: false,
        injection: pipeline::Injection::None,
    };
    let mut drill = DrillParts::default();
    let mut port_text: Option<String> = None;
    let mut injection_text: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].clone();
        let mut take = |name: &str| -> Result<String, String> {
            index += 1;
            args.get(index)
                .cloned()
                .ok_or_else(|| format!("{name} requires a value"))
        };
        match flag.as_str() {
            "--data-root" => config.data_root = PathBuf::from(take("--data-root")?),
            "--workspace" => config.workspace = take("--workspace")?,
            "--actor" => config.actor = take("--actor")?,
            "--profile" => config.profile = take("--profile")?,
            "--provision" => config.provision = Some(PathBuf::from(take("--provision")?)),
            "--token-file" => config.token_file = PathBuf::from(take("--token-file")?),
            "--control-port" => port_text = Some(take("--control-port")?),
            "--halt-file" => config.halt_file = Some(PathBuf::from(take("--halt-file")?)),
            "--keeper" => config.keeper = Some(PathBuf::from(take("--keeper")?)),
            "--render-worker" => {
                config.render_worker = Some(PathBuf::from(take("--render-worker")?))
            }
            "--media-drill" => drill.enabled = true,
            "--pcm" => drill.pcm = Some(PathBuf::from(take("--pcm")?)),
            "--avatar" => drill.manifest = Some(PathBuf::from(take("--avatar")?)),
            "--avatar-config" => drill.config = Some(PathBuf::from(take("--avatar-config")?)),
            "--asset" => {
                let raw = take("--asset")?;
                let (id, path) = raw
                    .split_once('=')
                    .ok_or_else(|| "--asset requires <id=path>".to_string())?;
                if id.is_empty() || path.is_empty() {
                    return Err("--asset requires <id=path>".into());
                }
                drill.assets.push((id.to_string(), PathBuf::from(path)));
            }
            "--license" => drill.licenses.push(take("--license")?),
            "--slot-bytes" => drill.slot_bytes = parse_u32("--slot-bytes", &take("--slot-bytes")?)?,
            "--time-base-den" => {
                drill.time_base_den = parse_u32("--time-base-den", &take("--time-base-den")?)?
            }
            "--event-file" => config.event_file = Some(PathBuf::from(take("--event-file")?)),
            "--rehearsal-provider" => config.rehearsal_provider = true,
            "--inject" => injection_text = Some(take("--inject")?),
            other => return Err(format!("unknown argument {other}")),
        }
        index += 1;
    }
    if config.data_root.as_os_str().is_empty()
        || config.workspace.is_empty()
        || config.actor.is_empty()
        || config.profile.is_empty()
    {
        return Err("--data-root, --workspace, --actor and --profile are required".into());
    }
    if config.provision.is_none() && config.token_file.as_os_str().is_empty() {
        return Err("--token-file is required unless --provision is given".into());
    }
    config.control_port = match port_text {
        Some(text) => text
            .parse()
            .map_err(|_| "--control-port must be a port number".to_string())?,
        None => 0,
    };
    if config.keeper.is_some() != config.render_worker.is_some() {
        return Err("--keeper and --render-worker must be given together".into());
    }
    if drill.enabled {
        let (Some(pcm), Some(avatar_manifest), Some(avatar_config)) =
            (drill.pcm, drill.manifest, drill.config)
        else {
            return Err("--media-drill requires --pcm, --avatar and --avatar-config".into());
        };
        if drill.assets.is_empty() || drill.licenses.is_empty() {
            return Err("--media-drill requires --asset and --license entries".into());
        }
        if config.keeper.is_none() {
            return Err("--media-drill requires --keeper and --render-worker".into());
        }
        config.media_drill = Some(MediaDrill {
            pcm,
            avatar_manifest,
            avatar_config,
            assets: drill.assets,
            licenses: drill.licenses,
            slot_bytes: if drill.slot_bytes == 0 {
                1_048_576
            } else {
                drill.slot_bytes
            },
            time_base_den: if drill.time_base_den == 0 {
                8000
            } else {
                drill.time_base_den
            },
        });
    }
    if let Some(text) = injection_text {
        config.injection = pipeline::Injection::parse(&text)?;
    }
    if config.event_file.is_some() && !config.rehearsal_provider {
        // The real provider transport is not wired for the manual-event
        // pipeline: without an explicit rehearsal generator the engine refuses
        // rather than silently standing in a stub for a model.
        return Err("--event-file requires --rehearsal-provider".into());
    }
    if config.injection != pipeline::Injection::None && config.event_file.is_none() {
        return Err("--inject is only available with --event-file".into());
    }
    Ok(config)
}

fn parse_u32(name: &str, text: &str) -> Result<u32, String> {
    text.parse()
        .map_err(|_| format!("{name} must be a non-negative integer"))
}

/// Reads the single-use control token. The file must be owner-only, so a shared
/// credential cannot be created by accident.
pub fn read_token(path: &Path) -> Result<String, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)
            .map_err(|error| format!("token file unavailable: {error}"))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err("token file must be 0600 (owner-only)".into());
        }
    }
    let raw =
        fs::read_to_string(path).map_err(|error| format!("token file unreadable: {error}"))?;
    let token = raw.trim().to_string();
    if token.len() < 32
        || token.len() > 256
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err("token file must hold 32-256 url-safe characters".into());
    }
    Ok(token)
}

/// Assembles the runtime, starts the loopback control surface and supervises the
/// media children until an explicit halt request.
pub async fn run(config: Config) -> Result<(), String> {
    if let Some(profile_file) = config.provision.as_deref() {
        provision(&config, profile_file)?;
        println!(
            "nai-engine provisioned: {}/{}",
            config.workspace, config.profile
        );
        io::stdout().flush().map_err(|error| error.to_string())?;
        return Ok(());
    }
    let token = read_token(&config.token_file)?;
    let access = Arc::new(
        Access::new(&token, "local-operator", CONTROL_PERMISSIONS, CONTROL_TTL)
            .map_err(|error| error.to_string())?,
    );
    let composition = Composition::prepare(
        DataPaths::at(&config.data_root).map_err(|error| error.to_string())?,
        &config.workspace,
        &config.actor,
        &config.profile,
        false,
    )
    .map_err(|error| format!("composition failed: {error}"))?;
    let shared_session = composition.session();
    let emergency = composition.emergency();
    let approvals = composition.approvals().clone();
    let shared = Arc::new(Mutex::new(composition));
    // §42.3/§42.4 记忆面接线：控制面拿到的后端就是这个装配根本身。
    // 策略由装配点给定（本轮无配置项，用默认：观众长期记忆**默认关闭**）。
    let memory = Arc::new(MemoryAdapter::with_default_policy(shared.clone()));
    let api = Arc::new(
        ControlApi::with_shared_session(
            shared_session,
            emergency.clone(),
            access.clone(),
            Some(approvals),
        )
        .map_err(|error| format!("control surface rejected: {error}"))?
        .with_memory(memory),
    );
    let listener = TcpListener::bind(("127.0.0.1", config.control_port))
        .await
        .map_err(|error| format!("control listener failed: {error}"))?;
    let address = listener.local_addr().map_err(|error| error.to_string())?;
    println!("nai-engine control: http://{address}/api/v1");
    io::stdout().flush().map_err(|error| error.to_string())?;

    let stop = Arc::new(AtomicBool::new(false));
    let (stop_tx, stop_rx) = watch::channel(false);
    let server = tokio::spawn(async move { serve(listener, api, stop_rx).await });
    let supervision = match (&config.keeper, &config.render_worker) {
        (Some(keeper), Some(render_worker)) => {
            let binaries = Binaries {
                keeper: keeper.clone(),
                render_worker: render_worker.clone(),
            };
            let drill = config.media_drill.clone();
            let shared = shared.clone();
            let stop = stop.clone();
            Some(thread::spawn(move || {
                if let Err(error) = supervise(&shared, binaries, drill, &stop) {
                    eprintln!("nai-engine supervision stopped: {error}");
                }
            }))
        }
        _ => None,
    };

    // The explicit manual-event rehearsal runs once, in-process, against the
    // live composition. A stage failure applies the ordered halt inside the
    // pipeline and propagates, so the engine exits without a success line.
    if let Some(script) = config.event_file.clone() {
        let pipeline_config = pipeline::PipelineConfig {
            script,
            provider_ref: "rehearsal".into(),
            price_revision: "price-rehearsal-1".into(),
            max_output_chars: 512,
            injection: config.injection,
        };
        pipeline::run(&shared, &pipeline_config)?;
    }

    wait_for_halt(config.halt_file.as_deref()).await;
    access.revoke();
    // P0 does not wait for composition/storage/logging during shutdown.
    emergency.mute();
    stop.store(true, Ordering::Release);
    halt(&shared);
    stop.store(true, Ordering::Release);
    let _ = stop_tx.send(true);
    let _ = server.await;
    if let Some(handle) = supervision {
        let _ = handle.join();
    }
    Ok(())
}

/// Trusted-owner provisioning: workspace, runtime permissions and the runtime
/// profile. It runs only with the owner's explicit arguments, never from a model
/// or plugin path, and it refuses to overwrite an existing workspace.
fn provision(config: &Config, profile_file: &Path) -> Result<(), String> {
    use nai_storage::{ConfigKind, Operation};
    let raw = fs::read_to_string(profile_file)
        .map_err(|error| format!("runtime profile unreadable: {error}"))?;
    let profile: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("runtime profile is not JSON: {error}"))?;
    // Provisioning opens the store, which runs the whole migration chain on a
    // fresh data root, so it takes the bring-up budget rather than the per-call
    // one. The elapsed time is part of the failure text: a bare deadline tells
    // the next reader nothing about which step was slow.
    let started = Instant::now();
    let operation = Operation::new(BRINGUP_OPERATION).map_err(|error| error.to_string())?;
    let mut store = nai_storage::Store::open(
        DataPaths::at(&config.data_root).map_err(|error| error.to_string())?,
        &operation,
    )
    .map_err(|error| {
        format!(
            "provisioning store open failed after {}ms: {error}",
            started.elapsed().as_millis()
        )
    })?;
    store
        .create_workspace(&operation, &config.workspace, &config.workspace)
        .map_err(|error| format!("workspace creation failed: {error}"))?;
    for (permission, resource) in [
        ("config.read", config.profile.as_str()),
        ("config.write", config.profile.as_str()),
        ("approval.read", "*"),
        ("approval.write", "*"),
        ("session.manage", "*"),
    ] {
        store
            .set_permission(
                &operation,
                &config.workspace,
                &config.actor,
                permission,
                resource,
                true,
            )
            .map_err(|error| format!("permission grant failed: {error}"))?;
    }
    store
        .save_config(
            &operation,
            (&config.workspace, &config.actor),
            ConfigKind::Runtime,
            &config.profile,
            &profile,
            0,
        )
        .map_err(|error| format!("runtime profile rejected: {error}"))?;
    Ok(())
}

/// Operator halt: P0 mute first, then session revocation, then the critical
/// cancellation entry. A log failure is reported and never rolls back the mute.
fn halt(shared: &Arc<Mutex<Composition>>) {
    let mut guard = match shared.lock() {
        Ok(guard) => guard,
        Err(_) => {
            eprintln!("nai-engine halt skipped: composition lock poisoned");
            return;
        }
    };
    if let Err(error) = guard.control(ControlAction::Mute) {
        eprintln!("nai-engine halt: media muted, logging failed: {error}");
    }
}

/// Supervises the keeper (and optionally runs the explicit media drill) until a
/// fault, a revoke, or the stop flag. Renewal stops after a revoke: re-granting
/// output authority needs an explicit operator decision, so it never happens
/// automatically here.
fn supervise(
    shared: &Arc<Mutex<Composition>>,
    binaries: Binaries,
    drill: Option<MediaDrill>,
    stop: &AtomicBool,
) -> Result<(), String> {
    let (session, renew_ms, floor) = {
        let composition = shared
            .lock()
            .map_err(|_| "composition lock poisoned".to_string())?;
        let floor = composition
            .session_lock()
            .map_err(|error| error.to_string())?
            .snapshot()
            .generation_epoch;
        (
            composition.session_id().to_string(),
            composition.lease_renew_ms(),
            floor,
        )
    };
    let mut supervisor = Supervisor::new(binaries).map_err(|error| error.to_string())?;
    let lease_epoch = floor.checked_add(1).ok_or("engine epoch exhausted")?;
    let lease_id = format!("engine-{lease_epoch}");
    let emergency = shared
        .lock()
        .map_err(|_| "composition lock poisoned")?
        .emergency();
    let mut guard = DrillGuard {
        shared,
        stop,
        session: &session,
        floor,
        epoch: lease_epoch,
        lease: &lease_id,
        renew_ms,
        next_renew: Instant::now(),
        armed: false,
    };
    let result = (|| {
        let mut media = match drill.as_ref() {
            Some(d) => Some(MediaSetup::create(&session, lease_epoch, d)?),
            None => None,
        };
        let frame_args = match media.as_ref() {
            Some(m) => {
                let spec = m.broker.owner().spec();
                Some(FramePoolArgs {
                    epoch: lease_epoch,
                    broker: Some(m.socket_path()),
                    pool_id: spec.pool_id.clone(),
                    producer: spec.producer_ref.clone(),
                    consumer: spec.consumer_ref.clone(),
                    slots: spec.slots,
                    slot_bytes: spec.slot_bytes,
                    file: m.broker.consumer_spawn_handle().map_err(contract_text)?,
                })
            }
            None => None,
        };
        supervisor
            .spawn_keeper(&session, floor, frame_args)
            .map_err(|e| e.to_string())?;
        if let Some(m) = media.as_mut() {
            declare(
                &mut m.broker,
                PeerRole::Consumer,
                supervisor.keeper_pid().ok_or("keeper pid missing")?,
            )?;
            await_grant(
                &mut m.broker,
                PeerRole::Consumer,
                &mut supervisor,
                &mut guard,
            )?;
        }
        supervisor
            .arm(&session, rehearsal(shared, &lease_id, lease_epoch)?)
            .map_err(|e| e.to_string())?;
        guard.armed = true;
        guard.next_renew = Instant::now() + Duration::from_millis(renew_ms);
        telemetry(shared, "armed", json!({"epoch":lease_epoch,"floor":floor}))?;
        if let (Some(d), Some(m)) = (drill.as_ref(), media.as_mut()) {
            let summary =
                run_media_drill(&mut supervisor, &session, lease_epoch, d, m, &mut guard)?;
            telemetry(shared, "media_drill_committed", summary)?;
        }
        loop {
            guard.tick(&mut supervisor)?;
            sleep_bounded(DRILL_POLL.as_millis() as u64, stop);
        }
    })();
    if let Err(error) = &result {
        emergency.mute();
        let live = live_epoch(shared).ok().flatten().unwrap_or(floor);
        if error != "HALT" && error != "REVOKED" && live == floor {
            halt(shared);
        }
        if error != "HALT" && error != "REVOKED" {
            let _ = telemetry(shared, "media_drill_failed", json!({"detail":error}));
        }
    }
    supervisor.shutdown().map_err(|e| e.to_string())?;
    match result {
        Err(e) if e == "HALT" || e == "REVOKED" => Ok(()),
        other => other,
    }
}

/// Reads the live generation epoch through the composition. `Ok(None)` means the
/// session lock stayed busy for the poll budget -- "not observed this tick" --
/// which the watch must treat as neither a change nor a fault. The composition
/// owns that distinction so the engine cannot drift from it.
fn live_epoch(shared: &Arc<Mutex<Composition>>) -> Result<Option<u64>, String> {
    let root = shared.lock().map_err(|_| "composition lock poisoned")?;
    root.live_epoch().map_err(|error| error.to_string())
}
struct DrillGuard<'a> {
    shared: &'a Arc<Mutex<Composition>>,
    stop: &'a AtomicBool,
    session: &'a str,
    floor: u64,
    epoch: u64,
    lease: &'a str,
    renew_ms: u64,
    next_renew: Instant,
    armed: bool,
}
impl DrillGuard<'_> {
    fn tick(&mut self, supervisor: &mut Supervisor) -> Result<(), String> {
        if self.stop.load(Ordering::Acquire) {
            return Err("HALT".into());
        }
        // A transient session-lock contention must not end the watch. A busy lock
        // reads as "not observed this tick" and the fault observe below still
        // runs, so a killed keeper is handled even when the epoch was unreadable.
        if let Some(live) = live_epoch(self.shared)?
            && live != self.floor
        {
            telemetry(
                self.shared,
                "renewal_halted_after_revoke",
                json!({"epoch":live}),
            )?;
            return Err("REVOKED".into());
        }
        if self.armed && Instant::now() >= self.next_renew {
            // Renewal is periodic by design, so an attempt that cannot be built or
            // sent is reported and retried on the next interval. Ending the watch
            // here would drop fault handling over a transient contention -- the one
            // outcome the loop must never produce instead of a revoke.
            match rehearsal(self.shared, self.lease, self.epoch) {
                Ok(lease) => {
                    if let Err(e) = supervisor.renew(self.session, lease)
                        && let Err(log) = telemetry(
                            self.shared,
                            "renewal_failed",
                            json!({"detail":e.to_string()}),
                        )
                    {
                        eprintln!("nai-engine renewal log failed: {log}");
                    }
                }
                Err(detail) => {
                    if let Err(log) =
                        telemetry(self.shared, "renewal_skipped", json!({"detail":detail}))
                    {
                        eprintln!("nai-engine skipped-renewal log failed: {log}");
                    }
                }
            }
            self.next_renew = Instant::now() + Duration::from_millis(self.renew_ms.max(1));
        }
        if self
            .shared
            .lock()
            .map_err(|_| "composition lock poisoned")?
            .observe(supervisor)
            .map_err(|e| e.to_string())?
            .is_some()
        {
            return Err("media child fault".into());
        }
        Ok(())
    }
    fn wait(
        &mut self,
        supervisor: &mut Supervisor,
        deadline: Instant,
        mut ready: impl FnMut(&mut Supervisor) -> Result<bool, String>,
    ) -> Result<(), String> {
        loop {
            self.tick(supervisor)?;
            if Instant::now() >= deadline {
                return Err("media preparation/slot deadline expired".into());
            }
            if ready(supervisor)? {
                return Ok(());
            }
            thread::sleep(DRILL_POLL);
        }
    }
}
struct MediaSetup {
    broker: FrameBroker,
    ram: tempfile::TempDir,
}
impl MediaSetup {
    fn create(session: &str, epoch: u64, d: &MediaDrill) -> Result<Self, String> {
        #[cfg(target_os = "linux")]
        {
            let ram = ram_dir()?;
            let broker = FrameBroker::bind(ram.path(), pool_spec(session, epoch, d)?, Vec::new())
                .map_err(contract_text)?;
            Ok(Self { broker, ram })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (session, epoch, d);
            Err("media drill requires Linux tmpfs".into())
        }
    }
    fn socket_path(&self) -> PathBuf {
        #[cfg(target_os = "linux")]
        {
            self.broker.socket_path().to_path_buf()
        }
        #[cfg(not(target_os = "linux"))]
        {
            PathBuf::new()
        }
    }
}
fn declare(broker: &mut FrameBroker, role: PeerRole, pid: u32) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        let uid = fs::metadata("/proc/self").map_err(|e| e.to_string())?.uid();
        broker
            .declare_peer(PeerIdentity {
                role,
                pid: i32::try_from(pid).map_err(|_| "pid overflow")?,
                uid,
            })
            .map_err(contract_text)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (broker, role, pid);
        Err("broker unavailable".into())
    }
}
fn await_grant(
    broker: &mut FrameBroker,
    role: PeerRole,
    supervisor: &mut Supervisor,
    guard: &mut DrillGuard<'_>,
) -> Result<(), String> {
    guard.wait(
        supervisor,
        Instant::now() + AVATAR_TIMEOUT,
        |_| match broker.poll_grants(Instant::now() + DRILL_POLL) {
            Ok(peer) => Ok(peer.role == role),
            Err(e) if e.code == nai_contracts::v1::ErrorCode::DeadlineExceeded as i32 => Ok(false),
            Err(e) => Err(contract_text(e)),
        },
    )
}

/// Builds a lease strictly above the keeper's floor, signed for authority the
/// live session still holds.
fn rehearsal(
    shared: &Arc<Mutex<Composition>>,
    lease_id: &str,
    epoch: u64,
) -> Result<nai_contracts::v1::PlaybackLease, String> {
    let composition = shared
        .lock()
        .map_err(|_| "composition lock poisoned".to_string())?;
    composition
        .rehearsal_lease(lease_id, &["engine-idle".to_string()], epoch, utc_ms())
        .map_err(|error| error.to_string())
}

fn telemetry(shared: &Arc<Mutex<Composition>>, event: &str, detail: Value) -> Result<(), String> {
    let mut composition = shared
        .lock()
        .map_err(|_| "composition lock poisoned".to_string())?;
    let session = composition.session_id().to_string();
    let payload = json!({"event": event, "detail": detail});
    composition
        .record(
            DecisionKind::Observation,
            Some(&session),
            &payload,
            &format!("engine-{event}"),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn sleep_bounded(milliseconds: u64, stop: &AtomicBool) {
    let deadline = Instant::now() + Duration::from_millis(milliseconds.max(1));
    while Instant::now() < deadline {
        if stop.load(Ordering::Acquire) {
            return;
        }
        thread::sleep(DRILL_POLL.min(deadline.saturating_duration_since(Instant::now())));
    }
}

/// Runs the explicit data-plane drill: bounded pool, warmed render-worker,
/// reserved ticket, owner commit, and an owned-pixel read through the consumer.
/// The keeper keeps its watchdog role; the frame path is not wired to it here.
fn run_media_drill(
    supervisor: &mut Supervisor,
    session: &str,
    epoch: u64,
    drill: &MediaDrill,
    media: &mut MediaSetup,
    guard: &mut DrillGuard<'_>,
) -> Result<Value, String> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (supervisor, session, epoch, drill, media, guard);
        Err("the media drill requires the Linux tmpfs backend".into())
    }
    #[cfg(target_os = "linux")]
    {
        let samples = pcm_samples(&drill.pcm)?;
        let metadata = frame_metadata(session, epoch, drill, samples)?;
        let ticket_path = media.ram.path().join("ticket.bin");
        let ready_path = media.ram.path().join("ready");
        let finish_path = media.ram.path().join("finish");
        let worker_spec = RenderWorkerSpec {
            broker: Some(media.socket_path()),
            finish: Some(finish_path.clone()),
            session: session.to_string(),
            epoch,
            pcm: drill.pcm.clone(),
            ticket: ticket_path.clone(),
            manifest: drill.avatar_manifest.clone(),
            config: drill.avatar_config.clone(),
            assets: drill.assets.clone(),
            licenses: drill.licenses.clone(),
            ready: Some(ready_path.clone()),
        };
        supervisor
            .spawn_worker(
                &worker_spec,
                media
                    .broker
                    .producer_spawn_handle()
                    .map_err(contract_text)?,
            )
            .map_err(|error| format!("render-worker spawn failed: {error}"))?;
        declare(
            &mut media.broker,
            PeerRole::Producer,
            supervisor.worker_pid().ok_or("worker pid missing")?,
        )?;
        guard.wait(supervisor, Instant::now() + AVATAR_TIMEOUT, |_| {
            Ok(ready_path.is_file())
        })?;
        await_grant(&mut media.broker, PeerRole::Producer, supervisor, guard)?;
        telemetry(
            guard.shared,
            "media_drill_granted",
            json!({"producer_pid":supervisor.worker_pid(),"consumer_pid":supervisor.keeper_pid(),"pool_id":media.broker.owner().spec().pool_id}),
        )?;
        let slot_deadline = Instant::now() + nai_media_core::frame_slots::MAX_BUDGET;
        let ticket = media
            .broker
            .reserve(metadata, Instant::now())
            .map_err(contract_text)?;
        deliver_ticket(&ticket, &ticket_path)?;

        let mut started = None;
        guard.wait(supervisor, slot_deadline, |s| {
            started = s.try_worker_event().map_err(|e| e.to_string())?;
            Ok(started.is_some())
        })?;
        let started = started.unwrap();
        if started.event != Some(Event::WriteStarted(true)) {
            return Err("render-worker did not open its slot".into());
        }
        let mut committed = None;
        guard.wait(supervisor, slot_deadline, |s| {
            committed = s.try_worker_event().map_err(|e| e.to_string())?;
            Ok(committed.is_some())
        })?;
        let committed = committed.unwrap();
        let Some(Event::Committed(commit)) = committed.event.clone() else {
            return Err("render-worker did not commit a frame".into());
        };
        media
            .broker
            .commit(commit.as_ref().clone(), Instant::now())
            .map_err(contract_text)?;
        media
            .broker
            .begin_consume(&ticket, Instant::now())
            .map_err(contract_text)?;
        guard.tick(supervisor)?;
        let expected_hash = commit.sha256.clone();
        let bytes = commit.byte_length;
        let seq = supervisor
            .submit_frame(
                session,
                FrameSlotEvent {
                    event: Some(Event::Committed(commit)),
                },
            )
            .map_err(|e| e.to_string())?;
        let mut observation = None;
        guard.wait(supervisor, slot_deadline, |s| {
            if let Some(status) = s.status() {
                if status.last_command_seq >= seq
                    && status.presenting
                    && status.lease_valid
                    && !status.output_available
                    && status.epoch_floor == epoch
                    && status.consumed_buffer_ref
                        == nai_media_core::frame_slots::buffer_ref(&ticket)
                    && status.consumed_sha256 == expected_hash
                {
                    observation = Some(status);
                    return Ok(true);
                }
                if status.last_command_seq >= seq && !status.presenting {
                    return Err(format!("keeper rejected frame: {}", status.standby_reason));
                }
            }
            Ok(false)
        })?;
        media
            .broker
            .release(&ticket, Instant::now())
            .map_err(contract_text)?;
        fs::write(finish_path, b"finished").map_err(|e| e.to_string())?;
        let status = supervisor
            .wait_worker(Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("render-worker exited unsuccessfully after commit".into());
        }
        Ok(json!({
            "buffer_ref": nai_media_core::frame_slots::buffer_ref(&ticket),
            "digest": expected_hash,
            "bytes": bytes,
            "epoch": epoch,
            "keeper_presenting": observation.unwrap().presenting,
            "output_available": false,
            "broker_declared_peers_verified": true,
        }))
    }
}

fn pool_spec(session: &str, epoch: u64, drill: &MediaDrill) -> Result<PoolSpec, String> {
    let spec = PoolSpec {
        pool_id: "engine-pool".into(),
        session_id: session.into(),
        generation_epoch: epoch,
        producer_ref: PRODUCER.into(),
        consumer_ref: CONSUMER.into(),
        slots: 1,
        slot_bytes: drill.slot_bytes,
    };
    spec.validate().map_err(contract_text)?;
    Ok(spec)
}

fn frame_metadata(
    session: &str,
    epoch: u64,
    drill: &MediaDrill,
    samples: u32,
) -> Result<FrameMetadata, String> {
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(&drill.avatar_manifest)
            .map_err(|error| format!("avatar manifest unreadable: {error}"))?,
    )
    .map_err(|error| format!("avatar manifest is not JSON: {error}"))?;
    let width = manifest["output_width"]
        .as_u64()
        .ok_or_else(|| "avatar manifest has no output_width".to_string())?;
    let height = manifest["output_height"]
        .as_u64()
        .ok_or_else(|| "avatar manifest has no output_height".to_string())?;
    Ok(FrameMetadata {
        session_id: session.into(),
        utterance_id: "engine-idle".into(),
        generation_epoch: epoch,
        width: width as u32,
        height: height as u32,
        pixel_format: "rgba8".into(),
        pts: 0,
        duration: samples,
        time_base_num: 1,
        time_base_den: drill.time_base_den,
        ..Default::default()
    })
}

fn pcm_samples(path: &Path) -> Result<u32, String> {
    let bytes = fs::metadata(path)
        .map_err(|error| format!("PCM unreadable: {error}"))?
        .len();
    if bytes == 0 || bytes % 4 != 0 || bytes / 4 > u32::MAX as u64 {
        return Err("PCM must hold whole f32 samples".into());
    }
    Ok((bytes / 4) as u32)
}

/// Atomically delivers the reserved ticket so the worker never reads a partial
/// file.
fn deliver_ticket(ticket: &FrameSlotTicket, path: &Path) -> Result<(), String> {
    let staging = path.with_extension("staging");
    fs::write(&staging, ticket.encode_to_vec()).map_err(|error| error.to_string())?;
    fs::rename(&staging, path).map_err(|error| error.to_string())
}

/// Private tmpfs directory for the frame pool. A missing tmpfs is an explicit
/// capability failure, not a silent fallback to disk.
#[cfg(target_os = "linux")]
fn ram_dir() -> Result<tempfile::TempDir, String> {
    let dir = tempfile::tempdir_in("/dev/shm")
        .map_err(|error| format!("frame pool requires /dev/shm: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    Ok(dir)
}

fn contract_text(error: nai_contracts::v1::ContractError) -> String {
    format!("code {}: {}", error.code, error.message)
}

fn utc_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

/// Bounded wait for the explicit halt request. Without a halt path the engine
/// runs until it is killed, which is never reported as a graceful shutdown.
async fn wait_for_halt(path: Option<&Path>) {
    let Some(path) = path else {
        std::future::pending::<()>().await;
        return;
    };
    while !path.exists() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
