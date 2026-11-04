//! RF01 narrow host: fixed child, ephemeral inherited credential, no legacy state/DB.
pub mod foundation;
pub mod public_output;
use nai_contracts::{workbench::*, workbench_wire as wire};
use prost::Message;
use std::{
    path::{Path, PathBuf},
    process::{Child, Stdio},
    sync::{Mutex, mpsc},
    time::{Duration, Instant},
};
static ACTIVE: Mutex<()> = Mutex::new(());
struct Reap(Child);
impl Drop for Reap {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
pub fn binary() -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("NAI_WORKBENCH_ENGINE") {
        let path = PathBuf::from(path);
        if path.is_absolute() && path.is_file() {
            return Ok(path);
        }
        return Err("WB_ENGINE_UNAVAILABLE".into());
    }
    let path = std::env::current_exe()
        .map_err(|_| "WB_ENGINE_UNAVAILABLE")?
        .with_file_name(format!(
            "nai-workbench-engine{}",
            std::env::consts::EXE_SUFFIX
        ));
    if !path.is_file() {
        return Err("WB_ENGINE_UNAVAILABLE".into());
    }
    Ok(path)
}
pub fn inspect_at(
    engine: &Path,
    data_root: &Path,
    presentation: i32,
    prepare: bool,
) -> Result<Vec<u8>, String> {
    let _guard = ACTIVE.try_lock().map_err(|_| "WB_BUSY")?;
    if !matches!(
        Presentation::try_from(presentation),
        Ok(Presentation::ContentOnly
            | Presentation::Voice
            | Presentation::Avatar
            | Presentation::HumanAssist)
    ) {
        return Err("WB_INVALID_ARGUMENT".into());
    }
    let mut raw = [0; 48];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut raw)
        .map_err(|_| "WB_AUTHENTICATION_FAILED")?;
    let token = zeroize::Zeroizing::new(
        raw[..32]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
    );
    let id = raw[32..]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    raw.fill(0);
    let config = WorkbenchConfig {
        edition: Edition::Lite as i32,
        deployment: Deployment::Local as i32,
        presentation,
        behavior: Behavior::Assisted as i32,
        delivery_scope: DeliveryScope::Private as i32,
        queue_limit: 2,
        task_deadline_ms: 3000,
        shadow: false,
    };
    let mut request = WorkbenchRequest {
        protocol_major: wire::VERSION,
        authorization: token.as_bytes().to_vec(),
        request_id: id.clone(),
        seq: 1,
        ttl_ms: wire::TTL_MS,
        operation: Some(if prepare {
            workbench_request::Operation::Prepare(WorkbenchPrepare {
                config: Some(config),
            })
        } else {
            workbench_request::Operation::Inspect(WorkbenchInspect {
                config: Some(config),
            })
        }),
    };
    let mut command = background_command(engine);
    command
        .arg("--data-root")
        .arg(data_root)
        .env("NAI_WORKBENCH_TOKEN", token.as_str())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = Reap(command.spawn().map_err(|_| "WB_ENGINE_SPAWN_FAILED")?);
    let mut input = child.0.stdin.take().ok_or("WB_PIPE_FAILED")?;
    let written = wire::write(&mut input, &request);
    request.authorization.fill(0);
    drop(input);
    written.map_err(|_| "WB_PIPE_FAILED")?;
    let output = child.0.stdout.take().ok_or("WB_PIPE_FAILED")?;
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut output = output;
        let result = wire::read::<WorkbenchResponse>(&mut output);
        let _ = tx.send(result);
    });
    let deadline = Instant::now() + Duration::from_millis(wire::TTL_MS as u64 + 1000);
    loop {
        match child.0.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err("WB_ENGINE_FAILED".into());
                }
                break;
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            Ok(None) => return Err("WB_DEADLINE_EXCEEDED".into()),
            Err(_) => return Err("WB_ENGINE_FAILED".into()),
        }
    }
    let response = rx
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| "WB_DEADLINE_EXCEEDED")?
        .map_err(|_| "WB_PROTOCOL_ERROR")?;
    if response.request_id != id
        || response.seq != 1
        || !response.handshake.as_ref().is_some_and(|h| {
            h.protocol_major == wire::VERSION && h.max_frame_bytes == wire::FRAME_LIMIT as u32
        })
    {
        return Err("WB_PROTOCOL_INCOMPATIBLE".into());
    }
    if let Some(workbench_response::Outcome::Problem(problem)) = &response.outcome {
        return Err(format!(
            "WB_{}",
            WorkbenchError::try_from(problem.code)
                .map_err(|_| "WB_PROTOCOL_ERROR")?
                .as_str_name()
        ));
    }
    let Some(workbench_response::Outcome::Capabilities(c)) = &response.outcome else {
        return Err("WB_PROTOCOL_ERROR".into());
    };
    if c.config != Some(config)
        || c.business_ready
        || c.started_media_processes != 0
        || c.provider_calls != 0
        || c.status
            != if prepare {
                WorkbenchReadiness::NotReady
            } else {
                WorkbenchReadiness::ControlAvailable
            } as i32
        || c.capabilities.is_empty()
        || c.capabilities.iter().any(|v| {
            v.capability_id.is_empty()
                || !matches!(
                    WorkbenchImplementation::try_from(v.implementation),
                    Ok(WorkbenchImplementation::Implemented | WorkbenchImplementation::NotStarted)
                )
                || v.available && v.capability_id != "workbench.inspect@1"
        })
        || c.capabilities
            .iter()
            .map(|v| &v.capability_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != c.capabilities.len()
        || c.capabilities.len() > 64
        || c.required_capabilities.len() > 64
    {
        return Err("WB_PROTOCOL_ERROR".into());
    }
    Ok(response.encode_to_vec())
}

fn background_command(program: &Path) -> std::process::Command {
    let command = std::process::Command::new(program);
    #[cfg(windows)]
    let command = {
        use std::os::windows::process::CommandExt;
        let mut command = command;
        command.creation_flags(0x0800_0000);
        command
    };
    command
}

pub mod human_mic;
