//! Trusted local desktop owner. Never accepts an actor, path or credential from web content.
#[path = "live_auto.rs"]
mod live_auto;
use super::{Reap, background_command};
use nai_contracts::{workbench::*, workbench_wire as wire};
use prost::Message;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::mpsc,
    time::Duration,
};
use zeroize::Zeroize;
pub fn random_id() -> Result<String, String> {
    let mut raw = [0; 16];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut raw)
        .map_err(|_| "WB_RANDOM_FAILED")?;
    raw[6] = (raw[6] & 15) | 64;
    raw[8] = (raw[8] & 63) | 128;
    let h = raw.iter().map(|b| format!("{b:02x}")).collect::<String>();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    ))
}
/// Priority stop: never acquires the owner/database mutex or model worker.
pub fn stop_audition(root: &Path) -> Result<(), String> {
    write_stop(root, "audition.stop")
}
pub(super) fn write_stop(root: &Path, name: &str) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }
    no_links(root)?;
    let path = root.join(name);
    no_links(&path)?;
    let mut f = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&path)
        .map_err(|_| "WB_STOP_FAILED")?;
    f.write_all(random_id()?.as_bytes())
        .map_err(|_| "WB_STOP_FAILED".into())
}
pub fn audition_nonce(root: &Path) -> Result<Vec<u8>, String> {
    Ok(AuditionStopNonce {
        nonce: read_stop(root, "audition.stop")?,
    }
    .encode_to_vec())
}
pub(super) fn read_stop(root: &Path, name: &str) -> Result<String, String> {
    no_links(root)?;
    let path = root.join(name);
    no_links(&path)?;
    let nonce = if path.exists() {
        let m = fs::metadata(&path).map_err(|_| "WB_STOP_FAILED")?;
        if m.len() > 64 {
            return Err("WB_STOP_FAILED".into());
        }
        fs::read_to_string(path).map_err(|_| "WB_STOP_FAILED")?
    } else {
        String::new()
    };
    if !nonce.is_empty() && !uuid(&nonce) {
        return Err("WB_STOP_FAILED".into());
    }
    Ok(nonce)
}
fn uuid(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}
fn no_links(path: &Path) -> Result<(), String> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("WB_DATA_ROOT_REJECTED".into());
    }
    let mut prefix = PathBuf::new();
    for c in path.components() {
        prefix.push(c);
        match fs::symlink_metadata(&prefix) {
            Ok(m) => {
                if m.file_type().is_symlink() {
                    return Err("WB_DATA_ROOT_REJECTED".into());
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if m.file_attributes() & 0x400 != 0 {
                        return Err("WB_DATA_ROOT_REJECTED".into());
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err("WB_DATA_ROOT_REJECTED".into()),
        }
    }
    Ok(())
}
fn principal(identity: &Path, root: &Path) -> Result<String, String> {
    no_links(identity)?;
    no_links(root)?;
    if identity.exists() {
        if fs::metadata(identity)
            .map_err(|_| "WB_IDENTITY_REJECTED")?
            .len()
            > 1024
        {
            return Err("WB_IDENTITY_REJECTED".into());
        }
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(identity).map_err(|_| "WB_IDENTITY_REJECTED")?)
                .map_err(|_| "WB_IDENTITY_REJECTED")?;
        let id = value["principal"].as_str().ok_or("WB_IDENTITY_REJECTED")?;
        if !uuid(id) || value != serde_json::json!({"schema_version":1,"principal":id}) {
            return Err("WB_IDENTITY_REJECTED".into());
        }
        return Ok(id.into());
    }
    if root.exists()
        && fs::read_dir(root)
            .map_err(|_| "WB_DATA_ROOT_REJECTED")?
            .next()
            .is_some()
    {
        return Err("WB_IDENTITY_MISSING".into());
    }
    fs::create_dir_all(identity.parent().ok_or("WB_DATA_ROOT_REJECTED")?)
        .map_err(|_| "WB_IDENTITY_REJECTED")?;
    let id = random_id()?;
    let bytes = serde_json::to_vec(&serde_json::json!({"schema_version":1,"principal":id}))
        .map_err(|_| "WB_IDENTITY_REJECTED")?;
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(identity)
        .map_err(|_| "WB_IDENTITY_REJECTED")?;
    f.write_all(&bytes).map_err(|_| "WB_IDENTITY_REJECTED")?;
    f.sync_all().map_err(|_| "WB_IDENTITY_REJECTED")?;
    Ok(id)
}
pub fn binary() -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    if let Some(p) = std::env::var_os("NAI_FOUNDATION_ENGINE") {
        let p = PathBuf::from(p);
        if p.is_absolute() && p.is_file() {
            return Ok(p);
        }
        return Err("WB_ENGINE_UNAVAILABLE".into());
    }
    let p = std::env::current_exe()
        .map_err(|_| "WB_ENGINE_UNAVAILABLE")?
        .with_file_name(format!(
            "nai-foundation-engine{}",
            std::env::consts::EXE_SUFFIX
        ));
    if p.is_file() {
        Ok(p)
    } else {
        Err("WB_ENGINE_UNAVAILABLE".into())
    }
}
pub struct Owner {
    child: Reap,
    input: mpsc::SyncSender<FoundationRequest>,
    output: mpsc::Receiver<Result<FoundationResponse, WorkbenchError>>,
    token: zeroize::Zeroizing<String>,
    seq: u64,
    execution: Option<FoundationPublicWindowPrepare>,
    automation: Option<live_auto::Coordinator>,
}
impl Owner {
    pub fn healthy(&mut self) -> bool {
        self.child.0.try_wait().ok() == Some(None)
    }
    pub fn list_workspaces(&mut self) -> Result<Vec<u8>, String> {
        self.call(
            &FoundationCommand {
                idempotency_key: random_id()?,
                operation: Some(foundation_command::Operation::ListWorkspaces(
                    FoundationBrowse {
                        limit: 32,
                        ..Default::default()
                    },
                )),
            }
            .encode_to_vec(),
        )
    }
    pub fn open_at(engine: &Path, identity: &Path, root: &Path) -> Result<Self, String> {
        Self::open_with_route(engine, identity, root, false)
    }
    #[cfg(debug_assertions)]
    pub fn open_diagnostic_route_at(
        engine: &Path,
        identity: &Path,
        root: &Path,
        direct: bool,
    ) -> Result<Self, String> {
        Self::open_with_route(engine, identity, root, direct)
    }
    fn open_with_route(
        engine: &Path,
        identity: &Path,
        root: &Path,
        direct: bool,
    ) -> Result<Self, String> {
        let principal = principal(identity, root)?;
        let mut raw = [0; 32];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut raw)
            .map_err(|_| "WB_RANDOM_FAILED")?;
        let token =
            zeroize::Zeroizing::new(raw.iter().map(|b| format!("{b:02x}")).collect::<String>());
        raw.fill(0);
        let mut command = background_command(engine);
        command.env_remove("NAI_TTS_DIAGNOSTIC_DIRECT");
        if direct {
            #[cfg(debug_assertions)]
            command.env("NAI_TTS_DIAGNOSTIC_DIRECT", "1");
        }
        command.arg("--data-root").arg(root).arg("--desktop-owner");
        if !root.join("foundation-root.json").exists() {
            command.arg("--initialize");
        }
        command
            .env("NAI_WORKBENCH_TOKEN", token.as_str())
            .env("NAI_FOUNDATION_PRINCIPAL", principal)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        let mut child = Reap(command.spawn().map_err(|_| "WB_ENGINE_SPAWN_FAILED")?);
        let mut input = child.0.stdin.take().ok_or("WB_PIPE_FAILED")?;
        let mut output = child.0.stdout.take().ok_or("WB_PIPE_FAILED")?;
        let (tx, rx) = mpsc::sync_channel::<FoundationRequest>(1);
        std::thread::spawn(move || {
            while let Ok(mut r) = rx.recv() {
                let result = wire::write(&mut input, &r);
                if let Some(FoundationCommand {
                    operation: Some(foundation_command::Operation::SaveLlmModel(x)),
                    ..
                }) = r.command.as_mut()
                {
                    x.api_key.zeroize();
                }
                if let Some(FoundationCommand {
                    operation: Some(foundation_command::Operation::SaveTtsModel(x)),
                    ..
                }) = r.command.as_mut()
                {
                    x.api_key.zeroize();
                }
                r.authorization.fill(0);
                if result.is_err() {
                    break;
                }
            }
        });
        let (out, output_rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            loop {
                let r = wire::read::<FoundationResponse>(&mut output);
                let failed = r.is_err();
                if out.send(r).is_err() || failed {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            input: tx,
            output: output_rx,
            token,
            seq: 0,
            execution: None,
            automation: None,
        })
    }
    pub fn call(&mut self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let raw = self.send(page_command(bytes)?)?;
        let (clean, private) = strip_preparation(&raw)?;
        if private.is_some() {
            return Err("WB_PERMISSION_DENIED".into());
        }
        Ok(clean)
    }
    /// Native-root only: observation is obtained here from the fixed OS child, never page data.
    pub fn call_with_public_output(
        &mut self,
        bytes: &[u8],
        mut output: Option<&mut crate::public_output::OutputOwner>,
        root: &Path,
    ) -> Result<Vec<u8>, String> {
        let command = page_command(bytes)?;
        // Renew before ordinary page work, so a pause/status transaction does not
        // consume the residual lease left by the preceding supervisor tick.
        if self.automation.is_some()
            && matches!(
                command.operation,
                Some(
                    foundation_command::Operation::ControlLiveRun(_)
                        | foundation_command::Operation::ReadFaq(_)
                        | foundation_command::Operation::AskFaq(_)
                        | foundation_command::Operation::ReadQuestionPool(_)
                        | foundation_command::Operation::ReadRunQuestionPool(_)
                        | foundation_command::Operation::EnqueueQuestion(_)
                        | foundation_command::Operation::EnqueueRunQuestion(_)
                        | foundation_command::Operation::RunQuestion(_)
                        | foundation_command::Operation::CancelQuestion(_)
                        | foundation_command::Operation::ControlLiveQuestion(_)
                        | foundation_command::Operation::ReadLiveRun(_)
                        | foundation_command::Operation::ReadLiveSpeechPlan(_)
                        | foundation_command::Operation::ReadTtsTask(_)
                )
            )
            && let Some(o) = output.as_deref_mut()
        {
            self.renew_execution(o, root)?;
        }
        self.public_command(command, output, root)
    }
    fn public_command(
        &mut self,
        mut command: FoundationCommand,
        mut output: Option<&mut crate::public_output::OutputOwner>,
        root: &Path,
    ) -> Result<Vec<u8>, String> {
        if matches!(
            command.operation,
            Some(foundation_command::Operation::ControlLiveTakeover(_))
        ) {
            self.automation = None;
            self.execution = None;
            if let Some(o) = output.as_deref_mut() {
                o.clear_prepared()?;
                if o.current().audio_active {
                    crate::public_output::stop(root)?;
                    return Err("WB_AI_STOP_UNCONFIRMED".into());
                }
            }
            return self.send_pumping(command, output);
        }
        if let Some(foundation_command::Operation::StartLiveAuto(x)) = command.operation.as_mut() {
            let output = output.ok_or("WB_PUBLIC_OUTPUT_UNAVAILABLE")?;
            if self.automation.is_some() {
                return Err("WB_BUSY".into());
            }
            let status = output.poll()?;
            if status.terminal || !status.standby || status.audio_active {
                return Err("WB_PUBLIC_OUTPUT_UNAVAILABLE".into());
            }
            let visual = status
                .visual
                .as_ref()
                .ok_or("WB_PUBLIC_OUTPUT_UNAVAILABLE")?;
            x.output_id = status.output_id;
            x.visual_sha256 = visual.package_sha256.clone();
            x.license_sha256 = visual.license_sha256.clone();
            x.stop_nonce = read_stop(root, "public-output.stop")?;
            let scope = x.run.clone().ok_or("WB_INVALID_ARGUMENT")?;
            let raw = self.send_pumping(command, Some(&mut *output))?;
            let r = live_auto::result(&raw)?;
            let view = r.live_auto.ok_or("WB_PROTOCOL_ERROR")?;
            self.automation = Some(live_auto::Coordinator::new(scope, view));
            return Ok(raw);
        }
        if matches!(
            command.operation,
            Some(foundation_command::Operation::StopLiveAuto(_))
                | Some(foundation_command::Operation::ControlLiveQuestion(
                    LiveQuestionControl { cancel: true, .. }
                ))
        ) {
            self.automation = None;
            self.execution = None;
            let mut output = output;
            if let Some(output) = output.as_deref_mut() {
                let _ = output.clear_prepared();
            }
            return self.send_pumping(command, output);
        }
        let bytes = command.encode_to_vec();
        if !matches!(
            command.operation,
            Some(
                foundation_command::Operation::CheckPublicCandidate(_)
                    | foundation_command::Operation::ConfirmPublicApproval(_)
                    | foundation_command::Operation::PreparePublicWindow(_)
                    | foundation_command::Operation::StartPublicExecution(_)
                    | foundation_command::Operation::RenewPublicExecution(_)
                    | foundation_command::Operation::StopPublicExecution(_)
                    | foundation_command::Operation::RevokePublicApproval(_)
                    | foundation_command::Operation::StartLiveAuto(_)
                    | foundation_command::Operation::ControlLiveQuestion(_)
                    | foundation_command::Operation::ControlLiveTakeover(_)
                    | foundation_command::Operation::StopLiveAuto(_)
                    | foundation_command::Operation::ConfirmLiveAuto(_)
            )
        ) {
            let raw = self.send_pumping(command, output)?;
            let (clean, private) = strip_preparation(&raw)?;
            if private.is_some() {
                return Err("WB_PERMISSION_DENIED".into());
            }
            return Ok(clean);
        }
        let Some(output) = output else {
            return self.call(&bytes);
        };
        if matches!(
            command.operation,
            Some(
                foundation_command::Operation::RevokePublicApproval(_)
                    | foundation_command::Operation::StopPublicExecution(_)
            )
        ) {
            self.execution = None;
            let _ = output.clear_prepared();
            let raw = self.send_pumping(command, Some(&mut *output))?;
            let (clean, private) = strip_preparation(&raw)?;
            if private.is_some() {
                return Err("WB_PROTOCOL_ERROR".into());
            }
            let _ = output.clear_prepared();
            return Ok(clean);
        }
        let is_execution = matches!(
            command.operation,
            Some(
                foundation_command::Operation::StartPublicExecution(_)
                    | foundation_command::Operation::RenewPublicExecution(_)
            )
        );
        if let Some(foundation_command::Operation::StartPublicExecution(x)) =
            command.operation.as_ref()
        {
            self.execution = None;
            let prepare = FoundationCommand {
                idempotency_key: random_id()?,
                operation: Some(foundation_command::Operation::PreparePublicWindow(
                    x.clone(),
                )),
            };
            let raw = self.public_command(prepare, Some(&mut *output), root)?;
            let response =
                FoundationResponse::decode(raw.as_slice()).map_err(|_| "WB_PROTOCOL_ERROR")?;
            if !matches!(response.outcome,Some(foundation_response::Outcome::Result(ref r)) if r.public_window_prepared.as_ref().is_some_and(|p|p.device_ready))
            {
                return Ok(raw);
            }
        }
        if is_execution
            && !output
                .current()
                .window_prepared
                .as_ref()
                .is_some_and(|p| p.submission_complete)
        {
            output.fresh_execution()?;
        }
        let raw = self.send_pumping(
            FoundationCommand {
                idempotency_key: random_id()?,
                operation: Some(foundation_command::Operation::PreparePublicOutputBinding(
                    true,
                )),
            },
            Some(&mut *output),
        )?;
        let response =
            FoundationResponse::decode(raw.as_slice()).map_err(|_| "WB_PROTOCOL_ERROR")?;
        let challenge = match response.outcome {
            Some(foundation_response::Outcome::Result(r)) => {
                r.public_output_challenge.ok_or("WB_PROTOCOL_ERROR")?
            }
            Some(foundation_response::Outcome::Problem(p)) => {
                return Err(format!(
                    "WB_{}",
                    WorkbenchError::try_from(p.code)
                        .map_err(|_| "WB_PROTOCOL_ERROR")?
                        .as_str_name()
                ));
            }
            _ => return Err("WB_PUBLIC_OUTPUT_UNAVAILABLE".into()),
        };
        if challenge.challenge == 0 || challenge.ttl_ms != 250 {
            return Err("WB_PROTOCOL_ERROR".into());
        }
        let status = match output.poll() {
            Ok(s) if !s.terminal => s,
            _ => {
                self.execution = None;
                let _ = output.clear_prepared();
                return self.call(&bytes);
            }
        };
        let nonce = read_stop(root, "public-output.stop")?;
        let candidate = match command.operation.as_mut() {
            Some(foundation_command::Operation::CheckPublicCandidate(x)) => Some(x),
            Some(foundation_command::Operation::ConfirmPublicApproval(x)) => x.candidate.as_mut(),
            Some(foundation_command::Operation::ConfirmLiveAuto(x)) => x.candidate.as_mut(),
            Some(
                foundation_command::Operation::PreparePublicWindow(x)
                | foundation_command::Operation::StartPublicExecution(x)
                | foundation_command::Operation::RenewPublicExecution(x),
            ) => x.candidate.as_mut(),
            _ => None,
        };
        if let Some(x) = candidate {
            x.output_observation = Some(PublicOutputObservation {
                challenge: challenge.challenge,
                status: Some(status),
                stop_nonce: nonce,
            });
        }
        let intent = if let Some(
            foundation_command::Operation::StartPublicExecution(x)
            | foundation_command::Operation::RenewPublicExecution(x),
        ) = &command.operation
        {
            let mut x = x.clone();
            if let Some(c) = x.candidate.as_mut() {
                c.output_observation = None;
            }
            Some(x)
        } else {
            None
        };
        let raw = self.send_pumping(command, Some(&mut *output))?;
        let mut response =
            FoundationResponse::decode(raw.as_slice()).map_err(|_| "WB_PROTOCOL_ERROR")?;
        if let Some(foundation_response::Outcome::Result(r)) = response.outcome.as_mut() {
            if let Some(lease) = r.private_execution_lease.take() {
                if !is_execution || r.private_window_preparation.is_some() {
                    let _ = output.clear_prepared();
                    return Err("WB_PROTOCOL_ERROR".into());
                }
                match output.execute(*lease) {
                    Ok(p) => {
                        r.public_window_prepared = Some(Box::new(p));
                        self.execution = intent;
                    }
                    Err(e) => {
                        self.execution = None;
                        let _ = output.clear_prepared();
                        return Err(e);
                    }
                }
            } else if is_execution {
                self.execution = None;
                if !r
                    .public_window_prepared
                    .as_ref()
                    .is_some_and(|p| p.submission_complete)
                {
                    let _ = output.clear_prepared();
                }
            }
        } else if is_execution {
            self.execution = None;
            let _ = output.clear_prepared();
        }
        let (clean, private) = strip_preparation(&response.encode_to_vec())?;
        if let Some(preparation) = private {
            let ready = output.prepare(*preparation)?;
            let mut response =
                FoundationResponse::decode(clean.as_slice()).map_err(|_| "WB_PROTOCOL_ERROR")?;
            if let Some(foundation_response::Outcome::Result(r)) = response.outcome.as_mut() {
                r.public_window_prepared = Some(Box::new(ready));
            }
            return Ok(response.encode_to_vec());
        }
        Ok(clean)
    }
    pub fn renew_public_output(
        &mut self,
        output: &mut crate::public_output::OutputOwner,
        root: &Path,
    ) -> Result<PublicOutputStatus, String> {
        if let Err(error) = self.renew_execution(output, root) {
            if let Some(auto) = self.automation.take() {
                self.execution = None;
                let _ = output.clear_prepared();
                let _ = self.send(FoundationCommand {
                    idempotency_key: random_id()?,
                    operation: Some(foundation_command::Operation::StopLiveAuto(LiveAutoStop {
                        run: Some(auto.scope),
                        grant_id: auto.grant_id,
                        reason: "AUTHORITY_UNAVAILABLE".into(),
                    })),
                });
            }
            return Err(error);
        }
        if let Some(mut auto) = self.automation.take() {
            match auto.tick(self, output, root) {
                Ok(true) => self.automation = Some(auto),
                Ok(false) => (),
                Err(error) => {
                    eprintln!("LIVE_AUTO_STOP phase={} code={error}", auto.phase_name());
                    self.execution = None;
                    let _ = output.clear_prepared();
                    let _ = self.send(FoundationCommand {
                        idempotency_key: random_id()?,
                        operation: Some(foundation_command::Operation::StopLiveAuto(
                            LiveAutoStop {
                                run: Some(auto.scope.clone()),
                                grant_id: auto.grant_id.clone(),
                                reason: match error.strip_prefix("WB_") {
                                    Some("RESOURCE_EXHAUSTED") => "RESOURCE_EXHAUSTED",
                                    Some("TTS_FAILED") => "TTS_FAILED",
                                    Some("POLICY_BLOCKED") => "POLICY_BLOCKED",
                                    Some("MEDIA_NOT_READY") => "MEDIA_NOT_READY",
                                    Some("OUTPUT_CHANGED") => "OUTPUT_CHANGED",
                                    Some("AUTHORITY_UNAVAILABLE") => "AUTHORITY_UNAVAILABLE",
                                    _ => "AUTO_FAILED",
                                }
                                .into(),
                            },
                        )),
                    });
                }
            }
        }
        Ok(output.current())
    }
    fn renew_execution(
        &mut self,
        output: &mut crate::public_output::OutputOwner,
        root: &Path,
    ) -> Result<PublicOutputStatus, String> {
        let status = output.poll()?;
        let Some(intent) = self.execution.clone() else {
            return Ok(status);
        };
        if !status
            .window_prepared
            .as_ref()
            .is_some_and(|p| p.playing && !p.execution_stopped || p.submission_complete)
        {
            self.execution = None;
            if self.automation.is_some() {
                return Err("WB_PUBLIC_EXECUTION_STOPPED".into());
            }
            return Ok(status);
        }
        let c = FoundationCommand {
            idempotency_key: random_id()?,
            operation: Some(foundation_command::Operation::RenewPublicExecution(intent)),
        };
        let renewal = self
            .public_command(c, Some(&mut *output), root)
            .and_then(|raw| public_renewal_result(&raw));
        if let Err(error) = renewal {
            self.execution = None;
            let _ = output.clear_prepared();
            return Err(error);
        }
        Ok(output.current())
    }
    fn send(&mut self, command: FoundationCommand) -> Result<Vec<u8>, String> {
        self.send_pumping(command, None)
    }
    fn send_pumping(
        &mut self,
        command: FoundationCommand,
        mut output: Option<&mut crate::public_output::OutputOwner>,
    ) -> Result<Vec<u8>, String> {
        use foundation_command::Operation::*;
        let operation = command.operation.as_ref().ok_or("WB_INVALID_ARGUMENT")?;
        if self
            .child
            .0
            .try_wait()
            .map_err(|_| "WB_ENGINE_LOST")?
            .is_some()
        {
            return Err("WB_ENGINE_LOST".into());
        }
        self.seq = self
            .seq
            .checked_add(1)
            .filter(|s| *s <= 1_000_000)
            .ok_or("WB_SEQUENCE_EXHAUSTED")?;
        let id = random_id()?;
        self.input
            .try_send(FoundationRequest {
                protocol_major: wire::VERSION,
                authorization: self.token.as_bytes().to_vec(),
                request_id: id.clone(),
                seq: self.seq,
                ttl_ms: 4000,
                command: Some(command.clone()),
            })
            .map_err(|_| "WB_ENGINE_LOST")?;
        let read = matches!(operation, WorkspaceLifecycle(x) if x.action == WorkspaceAction::Inspect as i32)
            || matches!(
                operation,
                ReadObject(_)
                    | PreviewSharedModels(_)
                    | ReadSharedMigration(_)
                    | PreviewWorkspacePurge(_)
                    | ReadPublication(_)
                    | ListWorkspaces(_)
                    | ListPreparation(_)
                    | ReadPreview(_)
                    | ReadFaq(_)
                    | ReadLiveRun(_)
                    | ReadLiveSpeechPlan(_)
                    | ReadLiveAuto(_)
                    | ReadQuestionPool(_)
                    | ReadRunQuestionPool(_)
                    | ReadLlmSettings(_)
                    | CheckLlm(_)
                    | CheckPreviewSpeech(_)
                    | CheckPublicCandidate(_)
                    | PreparePublicOutputBinding(_)
                    | ReadTtsTask(_)
                    | ReadLlmTask(_)
            );
        let until = std::time::Instant::now() + Duration::from_millis(4500);
        let mut pump_error = None;
        let response = loop {
            match self.output.recv_timeout(Duration::from_millis(20)) {
                Ok(r) => break r.map_err(|_| "WB_PROTOCOL_ERROR")?,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if std::time::Instant::now() >= until {
                        return Err(if read {
                            "WB_DEADLINE_EXCEEDED"
                        } else {
                            "WB_OUTCOME_UNKNOWN"
                        }
                        .into());
                    }
                    // Health only. No execution challenge or media lease is renewed here.
                    if pump_error.is_none()
                        && let Some(output) = output.as_deref_mut()
                        && let Err(e) = output.poll()
                    {
                        pump_error = Some(e);
                    }
                }
                Err(_) => return Err("WB_ENGINE_LOST".into()),
            }
        };
        // Drain the outstanding reply before returning a health error, preserving request correlation.
        if let Some(error) = pump_error {
            return Err(error);
        }
        if response.request_id != id || response.seq != self.seq || response.outcome.is_none() {
            return Err("WB_PROTOCOL_ERROR".into());
        }
        if let Some(foundation_response::Outcome::Problem(p)) = &response.outcome
            && WorkbenchError::try_from(p.code).is_err()
        {
            return Err("WB_PROTOCOL_ERROR".into());
        }
        Ok(response.encode_to_vec())
    }
}

fn public_renewal_result(raw: &[u8]) -> Result<(), String> {
    let r = FoundationResponse::decode(raw).map_err(|_| "WB_PROTOCOL_ERROR".to_owned())?;
    match r.outcome {
        Some(foundation_response::Outcome::Result(_)) => Ok(()),
        Some(foundation_response::Outcome::Problem(p)) => Err(format!(
            "WB_PUBLIC_RENEW_{}",
            WorkbenchError::try_from(p.code)
                .map_err(|_| "WB_PROTOCOL_ERROR")?
                .as_str_name()
        )),
        _ => Err("WB_PROTOCOL_ERROR".into()),
    }
}
pub fn needs_public_output(bytes: &[u8]) -> bool {
    bytes.len() <= 16384
        && FoundationCommand::decode(bytes).ok().is_some_and(|c| {
            matches!(
                c.operation,
                Some(
                    foundation_command::Operation::ControlLiveRun(_)
                        | foundation_command::Operation::ReadFaq(_)
                        | foundation_command::Operation::AskFaq(_)
                        | foundation_command::Operation::ReadQuestionPool(_)
                        | foundation_command::Operation::ReadRunQuestionPool(_)
                        | foundation_command::Operation::EnqueueQuestion(_)
                        | foundation_command::Operation::EnqueueRunQuestion(_)
                        | foundation_command::Operation::RunQuestion(_)
                        | foundation_command::Operation::CancelQuestion(_)
                        | foundation_command::Operation::ReadLiveAuto(_)
                        | foundation_command::Operation::ReadLiveRun(_)
                        | foundation_command::Operation::ReadLiveSpeechPlan(_)
                        | foundation_command::Operation::ReadTtsTask(_)
                        | foundation_command::Operation::CheckPublicCandidate(_)
                        | foundation_command::Operation::ConfirmPublicApproval(_)
                        | foundation_command::Operation::PreparePublicWindow(_)
                        | foundation_command::Operation::StartPublicExecution(_)
                        | foundation_command::Operation::RenewPublicExecution(_)
                        | foundation_command::Operation::StopPublicExecution(_)
                        | foundation_command::Operation::RevokePublicApproval(_)
                        | foundation_command::Operation::StartLiveAuto(_)
                        | foundation_command::Operation::ControlLiveQuestion(_)
                        | foundation_command::Operation::ControlLiveTakeover(_)
                        | foundation_command::Operation::StopLiveAuto(_)
                        | foundation_command::Operation::ConfirmLiveAuto(_)
                )
            )
        })
}
fn page_command(bytes: &[u8]) -> Result<FoundationCommand, String> {
    if bytes.len() > 16384 {
        return Err("WB_FRAME_LIMIT".into());
    }
    let mut command = FoundationCommand::decode(bytes).map_err(|_| "WB_PROTOCOL_ERROR")?;
    use foundation_command::Operation::*;
    if let Some(StartLiveAuto(x)) = command.operation.as_mut() {
        x.output_id.clear();
        x.visual_sha256.clear();
        x.license_sha256.clear();
        x.stop_nonce.clear();
    }
    let operation = command.operation.as_ref().ok_or("WB_INVALID_ARGUMENT")?;
    if !matches!(
        operation,
        CreateWorkspace(_)
            | PreviewSharedModels(_)
            | ReadSharedMigration(_)
            | MigrateSharedModels(_)
            | CancelSharedMigration(_)
            | PreviewWorkspacePurge(_)
            | PurgeWorkspace(_)
            | ReadWorkspacePurge(_)
            | WorkspaceContext(_)
            | CreateWorkspaceSettings(_)
            | EditWorkspaceSettings(_)
            | RenameWorkspace(_)
            | WorkspaceLifecycle(_)
            | PutPreparation(_)
            | ReadObject(_)
            | PublicationImpact(_)
            | Publish(_)
            | ReadPublication(_)
            | RevokePublication(_)
            | ListWorkspaces(_)
            | ListPreparation(_)
            | Preview(_)
            | ReadPreview(_)
            | PreviewPresented(_)
            | StopPreview(_)
            | ReadLiveRun(_)
            | ReadLiveSpeechPlan(_)
            | ReadLiveAuto(_)
            | ReadQuestionPool(_)
            | ReadRunQuestionPool(_)
            | PreviewLiveSentence(_)
            | ControlLiveBatch(_)
            | StartLiveAuto(_)
            | ControlLiveQuestion(_)
            | ControlLiveTakeover(_)
            | ReadReview(_)
            | SaveReviewNote(_)
            | PlanReviewDraft(_)
            | ReadReviewDraft(_)
            | ApplyReviewDraft(_)
            | StopLiveAuto(_)
            | CreateLiveRun(_)
            | ControlLiveRun(_)
            | EnqueueQuestion(_)
            | EnqueueRunQuestion(_)
            | RunQuestion(_)
            | CancelQuestion(_)
            | AskFaq(_)
            | ReadFaq(_)
            | ResolveFaq(_)
            | SaveTtsModel(_)
            | DeleteTtsModel(_)
            | DeleteLlmModel(_)
            | SaveLlmModel(_)
            | ReadLlmSettings(_)
            | SetLlmBudget(_)
            | StartTtsAudition(_)
            | SelectOutputDevice(_)
            | RenewTtsAudition(_)
            | StopTtsAudition(_)
            | AuthorizeTtsAudio(_)
            | RevokeTtsAudio(_)
            | TestTts(_)
            | CheckPreviewSpeech(_)
            | CheckPublicCandidate(_)
            | ConfirmPublicApproval(_)
            | RevokePublicApproval(_)
            | PreparePublicWindow(_)
            | StartPublicExecution(_)
            | StopPublicExecution(_)
            | SynthesizePreviewSpeech(_)
            | ReadTtsTask(_)
            | CancelTts(_)
            | TestLlm(_)
            | StartLlm(_)
            | CheckLlm(_)
            | ReadLlmTask(_)
            | CancelLlm(_)
            | ResolveLlm(_)
    ) {
        return Err("WB_PERMISSION_DENIED".into());
    }
    if let Some(foundation_command::Operation::CheckPublicCandidate(x)) = command.operation.as_mut()
    {
        x.output_observation = None;
    }
    if let Some(foundation_command::Operation::ConfirmPublicApproval(x)) =
        command.operation.as_mut()
        && let Some(c) = x.candidate.as_mut()
    {
        c.output_observation = None;
    }
    if let Some(
        foundation_command::Operation::PreparePublicWindow(x)
        | foundation_command::Operation::StartPublicExecution(x),
    ) = command.operation.as_mut()
        && let Some(c) = x.candidate.as_mut()
    {
        c.output_observation = None;
    }
    Ok(command)
}

fn strip_preparation(
    raw: &[u8],
) -> Result<(Vec<u8>, Option<Box<PublicWindowPreparation>>), String> {
    let mut response = FoundationResponse::decode(raw).map_err(|_| "WB_PROTOCOL_ERROR")?;
    let private = match response.outcome.as_mut() {
        Some(foundation_response::Outcome::Result(r)) => {
            if r.private_execution_lease.is_some() {
                return Err("WB_PERMISSION_DENIED".into());
            }
            r.private_window_preparation.take()
        }
        _ => None,
    };
    Ok((response.encode_to_vec(), private))
}

#[cfg(test)]
mod output_binding_tests {
    use super::*;
    #[test]
    fn page_cannot_prepare_or_confirm_auto_and_cannot_choose_native_output() {
        for operation in [
            foundation_command::Operation::PrepareLiveAuto(LiveAutoPrepare::default()),
            foundation_command::Operation::ConfirmLiveAuto(LiveAutoConfirm::default()),
        ] {
            let c = FoundationCommand {
                idempotency_key: random_id().unwrap(),
                operation: Some(operation),
            };
            assert!(page_command(&c.encode_to_vec()).is_err());
        }
        let c = FoundationCommand {
            idempotency_key: random_id().unwrap(),
            operation: Some(foundation_command::Operation::StartLiveAuto(
                LiveAutoStart {
                    output_id: "page-output".into(),
                    visual_sha256: "page-visual".into(),
                    license_sha256: "page-license".into(),
                    stop_nonce: "page-nonce".into(),
                    ..Default::default()
                },
            )),
        };
        let clean = page_command(&c.encode_to_vec()).unwrap();
        let Some(foundation_command::Operation::StartLiveAuto(x)) = clean.operation else {
            panic!("start")
        };
        assert!(
            x.output_id.is_empty()
                && x.visual_sha256.is_empty()
                && x.license_sha256.is_empty()
                && x.stop_nonce.is_empty()
        );
    }
    #[test]
    fn private_window_path_is_stripped_before_page_delivery() {
        let response = FoundationResponse {
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                private_window_preparation: Some(Box::new(PublicWindowPreparation {
                    data_path: "private-root/secret.pcm".into(),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        let (clean, cap) = strip_preparation(&response.encode_to_vec()).unwrap();
        assert_eq!(cap.unwrap().data_path, "private-root/secret.pcm");
        let decoded = FoundationResponse::decode(clean.as_slice()).unwrap();
        let Some(foundation_response::Outcome::Result(r)) = decoded.outcome else {
            panic!("result")
        };
        assert!(r.private_window_preparation.is_none());
        assert!(!String::from_utf8_lossy(&clean).contains("secret.pcm"));
        let command = FoundationCommand {
            operation: Some(foundation_command::Operation::PreparePublicWindow(
                FoundationPublicWindowPrepare {
                    candidate: Some(FoundationPublicCandidate {
                        output_observation: Some(Default::default()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )),
            ..Default::default()
        };
        let clean = page_command(&command.encode_to_vec()).unwrap();
        let Some(foundation_command::Operation::PreparePublicWindow(p)) = clean.operation else {
            panic!("command")
        };
        assert!(p.candidate.unwrap().output_observation.is_none());
    }
    #[test]
    fn execution_secret_and_native_renewal_cannot_cross_page_boundary() {
        let r = FoundationResponse {
            outcome: Some(foundation_response::Outcome::Result(FoundationResult {
                private_execution_lease: Some(Box::new(PublicExecutionLease {
                    authorization: "secret".into(),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(strip_preparation(&r.encode_to_vec()).is_err());
        let r = FoundationCommand {
            operation: Some(foundation_command::Operation::RenewPublicExecution(
                Default::default(),
            )),
            ..Default::default()
        };
        assert!(page_command(&r.encode_to_vec()).is_err());
        let r = FoundationCommand {
            operation: Some(foundation_command::Operation::StartPublicExecution(
                FoundationPublicWindowPrepare {
                    candidate: Some(FoundationPublicCandidate {
                        output_observation: Some(Default::default()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )),
            ..Default::default()
        };
        let c = page_command(&r.encode_to_vec()).unwrap();
        let Some(foundation_command::Operation::StartPublicExecution(c)) = c.operation else {
            panic!("start")
        };
        assert!(c.candidate.unwrap().output_observation.is_none());
    }
    #[test]
    fn confirmation_observation_is_native_only() {
        let cmd = FoundationCommand {
            operation: Some(foundation_command::Operation::ConfirmPublicApproval(
                FoundationPublicApproval {
                    candidate: Some(FoundationPublicCandidate {
                        output_observation: Some(PublicOutputObservation::default()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )),
            ..Default::default()
        };
        let clean = page_command(&cmd.encode_to_vec()).unwrap();
        let Some(foundation_command::Operation::ConfirmPublicApproval(c)) = clean.operation else {
            panic!("command")
        };
        assert!(c.candidate.unwrap().output_observation.is_none());
        assert!(needs_public_output(&cmd.encode_to_vec()));
    }
    #[test]
    fn page_cannot_supply_an_output_observation_or_request_a_private_challenge() {
        let fake = FoundationCommand {
            operation: Some(foundation_command::Operation::CheckPublicCandidate(
                FoundationPublicCandidate {
                    output_observation: Some(PublicOutputObservation {
                        challenge: 9,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )),
            ..Default::default()
        };
        let clean = page_command(&fake.encode_to_vec()).unwrap();
        let Some(foundation_command::Operation::CheckPublicCandidate(c)) = clean.operation else {
            panic!("wrong operation")
        };
        assert!(c.output_observation.is_none());
        let private = FoundationCommand {
            operation: Some(foundation_command::Operation::PreparePublicOutputBinding(
                true,
            )),
            ..Default::default()
        };
        assert_eq!(
            page_command(&private.encode_to_vec()).unwrap_err(),
            "WB_PERMISSION_DENIED"
        );
    }
    #[test]
    fn public_renewal_preserves_safe_denial_and_rejects_malformed_outcomes() {
        let encode = |outcome| {
            FoundationResponse {
                outcome,
                ..Default::default()
            }
            .encode_to_vec()
        };
        assert!(
            public_renewal_result(&encode(Some(foundation_response::Outcome::Result(
                FoundationResult::default()
            ))))
            .is_ok()
        );
        let denied = encode(Some(foundation_response::Outcome::Problem(
            WorkbenchProblem {
                code: WorkbenchError::DeadlineExceeded as i32,
                safe_message: "untrusted upstream detail must not escape".into(),
                ..Default::default()
            },
        )));
        assert_eq!(
            public_renewal_result(&denied).unwrap_err(),
            "WB_PUBLIC_RENEW_WORKBENCH_ERROR_DEADLINE_EXCEEDED"
        );
        assert_eq!(
            public_renewal_result(&encode(None)).unwrap_err(),
            "WB_PROTOCOL_ERROR"
        );
        assert_eq!(
            public_renewal_result(&[255]).unwrap_err(),
            "WB_PROTOCOL_ERROR"
        );
        let unknown = encode(Some(foundation_response::Outcome::Problem(
            WorkbenchProblem {
                code: i32::MAX,
                ..Default::default()
            },
        )));
        assert_eq!(
            public_renewal_result(&unknown).unwrap_err(),
            "WB_PROTOCOL_ERROR"
        );
    }
}
