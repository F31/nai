use super::*;
use nai_contracts::workbench::*;
use nai_workbench_host::human_mic::{self, MicOwner};
use prost::Message;
#[derive(Default)]
struct Inner {
    owner: Option<MicOwner>,
    last: Option<(MicStatus, Instant)>,
    authorized: bool,
    last_health: Option<Instant>,
}
#[derive(Clone, Default)]
pub struct MicService(Arc<Mutex<Inner>>);
fn encoded(s: &MicStatus) -> Vec<u8> {
    s.encode_to_vec()
}
fn stopped(inner: &mut Inner, reason: &str) {
    inner.authorized = false;
    inner.owner.take();
    if let Some((s, t)) = inner.last.as_mut() {
        s.active = false;
        s.reason = reason.into();
        *t = Instant::now();
    }
}
fn result(bytes: &[u8]) -> Result<FoundationResult, String> {
    match FoundationResponse::decode(bytes)
        .map_err(|_| "WB_PROTOCOL_ERROR")?
        .outcome
    {
        Some(foundation_response::Outcome::Result(r)) => Ok(r),
        Some(foundation_response::Outcome::Problem(p)) => Err(format!(
            "WB_{}",
            WorkbenchError::try_from(p.code)
                .map_err(|_| "WB_PROTOCOL_ERROR")?
                .as_str_name()
                .trim_start_matches("WORKBENCH_ERROR_")
        )),
        _ => Err("WB_PROTOCOL_ERROR".into()),
    }
}
pub fn stop_file(app: &tauri::AppHandle) -> Result<(), String> {
    human_mic::stop(&audition_root(app)?)
}
pub fn stop_owned(app: &tauri::AppHandle) -> Result<(), String> {
    stop_file(app)?;
    let service = app.state::<MicService>();
    if let Ok(mut m) = service.0.try_lock() {
        stopped(&mut m, "STOPPED");
    }
    Ok(())
}
#[tauri::command]
pub fn workbench_all_mute(app: tauri::AppHandle) -> Result<(), String> {
    let a = workbench_public_stop(app.clone());
    let b = stop_file(&app);
    let c = workbench_stop_audition(app);
    a.and(b).and(c)
}
#[tauri::command]
pub fn workbench_mic_stop(app: tauri::AppHandle) -> Result<(), String> {
    stop_file(&app)
}
#[tauri::command]
pub async fn workbench_mic_devices(
    app: tauri::AppHandle,
    state: tauri::State<'_, MicService>,
) -> Result<Vec<u8>, String> {
    let root = audition_root(&app)?;
    let shared = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut m = shared.try_lock().map_err(|_| "WB_MIC_BUSY")?;
        if m.owner.as_ref().is_some_and(|o| !o.status.active) {
            m.owner.take();
        }
        if m.owner.is_none() {
            m.owner = Some(MicOwner::open_at(&human_mic::binary()?, &root)?);
        }
        let s = m.owner.as_mut().ok_or("WB_MIC_UNAVAILABLE")?.poll()?;
        m.last = Some((s.clone(), Instant::now()));
        Ok(encoded(&s))
    })
    .await
    .map_err(|_| "WB_MIC_WORKER_LOST".to_string())?
}
#[tauri::command]
pub fn workbench_mic_poll(state: tauri::State<'_, MicService>) -> Result<Vec<u8>, String> {
    let m = state.0.try_lock().map_err(|_| "WB_MIC_BUSY")?;
    let (s, t) = m.last.as_ref().ok_or("WB_MIC_NOT_OPEN")?;
    if s.active && t.elapsed() >= Duration::from_millis(500) {
        return Err("WB_MIC_STATUS_EXPIRED".into());
    }
    Ok(encoded(s))
}
#[tauri::command]
pub async fn workbench_human_start(
    app: tauri::AppHandle,
    public: tauri::State<'_, PublicOutputOwner>,
    business: tauri::State<'_, BusinessOwner>,
    mic: tauri::State<'_, MicService>,
    takeover: Vec<u8>,
    route: Vec<u8>,
) -> Result<Vec<u8>, String> {
    if takeover.len() > 4096 || route.len() > 4096 {
        return Err("WB_FRAME_LIMIT".into());
    }
    let x = LiveTakeoverControl::decode(takeover.as_slice()).map_err(|_| "WB_PROTOCOL_ERROR")?;
    let r = MicControl::decode(route.as_slice()).map_err(|_| "WB_PROTOCOL_ERROR")?;
    if x.resume || !x.acknowledge || !r.acknowledge_route {
        return Err("WB_PERMISSION_DENIED".into());
    }
    let root = audition_root(&app)?;
    let p = public.inner().clone();
    let b = business.inner().clone();
    let m = mic.0.clone();
    b.1.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "WB_BUSY")?;
    let reservation = UserCommand(b.1.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let _reservation = reservation;
        let mut output = p.0.try_lock().map_err(|_| "WB_BUSY")?;
        let mut owner = b.0.try_lock().map_err(|_| "WB_BUSY")?;
        let mut mic = m.try_lock().map_err(|_| "WB_MIC_BUSY")?;
        let current = mic.owner.as_ref().ok_or("WB_MIC_NOT_OPEN")?;
        if current.status.instance_id != r.instance_id
            || current.status.active
            || !current
                .status
                .inputs
                .iter()
                .any(|d| d.device_id == r.input_id)
            || !current
                .status
                .outputs
                .iter()
                .any(|d| d.device_id == r.output_id)
        {
            return Err("WB_MIC_ROUTE_REJECTED".into());
        }
        if output.is_none() {
            return Err("WB_PUBLIC_OUTPUT_REQUIRED".into());
        }
        workbench_stop_audition(app.clone())?;
        let command = FoundationCommand {
            idempotency_key: nai_workbench_host::foundation::random_id()?,
            operation: Some(
                nai_contracts::workbench::foundation_command::Operation::ControlLiveTakeover(x),
            ),
        }
        .encode_to_vec();
        let raw = owner
            .as_mut()
            .ok_or("WB_NOT_CONNECTED")?
            .call_with_public_output(&command, output.as_mut(), &root)?;
        let c = result(&raw)?.live_context.ok_or("WB_PROTOCOL_ERROR")?;
        if !c.human_takeover {
            return Err("WB_PERMISSION_DENIED".into());
        }
        let started = mic
            .owner
            .as_mut()
            .ok_or("WB_MIC_NOT_OPEN")?
            .start_with_standby(
                &r.instance_id,
                r.input_id,
                r.output_id,
                true,
                output.as_mut().ok_or("WB_PUBLIC_OUTPUT_REQUIRED")?,
            );
        match started {
            Ok(s) => {
                mic.authorized = true;
                mic.last_health = Some(Instant::now());
                mic.last = Some((s, Instant::now()));
                Ok(raw)
            }
            Err(e) => {
                let _ = human_mic::stop(&root);
                stopped(&mut mic, "DEVICE_UNAVAILABLE");
                Err(e)
            }
        }
    })
    .await
    .map_err(|_| "WB_MIC_WORKER_LOST".to_string())?
}
#[tauri::command]
pub async fn workbench_human_resume(
    app: tauri::AppHandle,
    business: tauri::State<'_, BusinessOwner>,
    mic: tauri::State<'_, MicService>,
    takeover: Vec<u8>,
) -> Result<Vec<u8>, String> {
    if takeover.len() > 4096 {
        return Err("WB_FRAME_LIMIT".into());
    }
    let x = LiveTakeoverControl::decode(takeover.as_slice()).map_err(|_| "WB_PROTOCOL_ERROR")?;
    if !x.resume || !x.acknowledge {
        return Err("WB_PERMISSION_DENIED".into());
    }
    let root = audition_root(&app)?;
    let b = business.inner().clone();
    let m = mic.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        {
            let mut mic = m.try_lock().map_err(|_| "WB_MIC_BUSY")?;
            if let Some(owner) = mic.owner.as_mut()
                && owner.status.active
                && let Err(e) = owner.halt()
            {
                let _ = human_mic::stop(&root);
                stopped(&mut mic, "STOP_UNCONFIRMED");
                return Err(e);
            }
            mic.authorized = false;
            if let Some(o) = mic.owner.as_ref() {
                let status = o.status.clone();
                mic.last = Some((status, Instant::now()));
            }
        }
        let mut owner = b.0.try_lock().map_err(|_| "WB_BUSY")?;
        let command = FoundationCommand {
            idempotency_key: nai_workbench_host::foundation::random_id()?,
            operation: Some(
                nai_contracts::workbench::foundation_command::Operation::ControlLiveTakeover(x),
            ),
        }
        .encode_to_vec();
        let raw = owner.as_mut().ok_or("WB_NOT_CONNECTED")?.call(&command)?;
        result(&raw)?;
        Ok(raw)
    })
    .await
    .map_err(|_| "WB_MIC_WORKER_LOST".to_string())?
}
pub fn supervise(app: tauri::AppHandle, halt: Arc<AtomicBool>) {
    let service = app.state::<MicService>().0.clone();
    let business = app.state::<BusinessOwner>().inner().clone();
    let root = match audition_root(&app) {
        Ok(r) => r,
        Err(_) => return,
    };
    std::thread::spawn(move || {
        while !halt.load(Ordering::Acquire) {
            let next = Instant::now() + Duration::from_millis(100);
            let health = business
                .0
                .try_lock()
                .ok()
                .map(|mut b| b.as_mut().is_some_and(|o| o.healthy()));
            if let Ok(mut m) = service.try_lock() {
                if health == Some(true) {
                    m.last_health = Some(Instant::now());
                }
                if m.authorized
                    && (health == Some(false)
                        || m.last_health
                            .is_none_or(|t| t.elapsed() >= Duration::from_secs(5)))
                {
                    let _ = human_mic::stop(&root);
                    stopped(&mut m, "OWNER_LOST");
                }
                if let Some(o) = m.owner.as_mut() {
                    match o.poll() {
                        Ok(s) if !s.active || m.authorized => {
                            m.last = Some((s, Instant::now()));
                        }
                        _ => {
                            let _ = human_mic::stop(&root);
                            stopped(&mut m, "WORKER_OR_LEASE_LOST");
                        }
                    }
                }
            }
            std::thread::sleep(next.saturating_duration_since(Instant::now()));
        }
        let _ = human_mic::stop(&root);
        if let Ok(mut m) = service.lock() {
            stopped(&mut m, "OWNER_CLOSED");
        }
    });
}
