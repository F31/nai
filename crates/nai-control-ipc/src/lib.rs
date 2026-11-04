//! Private spawn-inherited control handles. No sockets, business rules or OS ACL claims.
use nai_contracts::v1::{LocalControlFrame as Frame, LocalControlKind as Kind};
use prost::Message;
use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

pub const VERSION: u32 = 1;
pub const FRAME_LIMIT: usize = 1024 * 1024;
pub const QUEUE_LIMIT: usize = 16;
pub const CALL_TIMEOUT: Duration = Duration::from_secs(4);
pub const PEER_TIMEOUT: Duration = Duration::from_secs(5);
pub const START_TIMEOUT: Duration = Duration::from_secs(45);
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);

pub fn frame(kind: Kind) -> Frame {
    Frame {
        version: VERSION,
        kind: kind as i32,
        ..Default::default()
    }
}

/// A truncated prefix is not a clean EOF. Length is checked before allocation.
pub fn read_frame(input: &mut impl Read) -> Result<Option<Frame>, String> {
    let mut size = [0; 4];
    match input.read(&mut size[..1]).map_err(|_| "IPC_READ_FAILED")? {
        0 => return Ok(None),
        _ => input
            .read_exact(&mut size[1..])
            .map_err(|_| "IPC_TRUNCATED_PREFIX")?,
    }
    let size = u32::from_be_bytes(size) as usize;
    if size == 0 || size > FRAME_LIMIT {
        return Err("IPC_FRAME_LIMIT".into());
    }
    let mut body = vec![0; size];
    input
        .read_exact(&mut body)
        .map_err(|_| "IPC_TRUNCATED_BODY")?;
    let value = Frame::decode(body.as_slice()).map_err(|_| "IPC_INVALID_PROTOBUF")?;
    if value.version != VERSION {
        return Err("IPC_VERSION_INCOMPATIBLE".into());
    }
    Ok(Some(value))
}

pub fn write_frame(output: &mut impl Write, value: &Frame) -> Result<(), String> {
    let size = value.encoded_len();
    if size == 0 || size > FRAME_LIMIT {
        return Err("IPC_FRAME_LIMIT".into());
    }
    output
        .write_all(&(size as u32).to_be_bytes())
        .map_err(|_| "IPC_WRITE_FAILED")?;
    output
        .write_all(&value.encode_to_vec())
        .map_err(|_| "IPC_WRITE_FAILED")?;
    output.flush().map_err(|_| "IPC_WRITE_FAILED".into())
}

/// Semantic shape checks are shared by both sides, not delegated to the WebView.
pub fn validate(value: &Frame) -> Result<Kind, String> {
    if value.version != VERSION {
        return Err("IPC_VERSION_INCOMPATIBLE".into());
    }
    let kind = Kind::try_from(value.kind).map_err(|_| "IPC_KIND_INVALID")?;
    let no_route = value.method.is_empty() && value.path.is_empty();
    let no_data = value.request_id.is_empty() && value.body.is_empty() && no_route;
    let no_auth = value.bootstrap_token.is_empty();
    let valid = match kind {
        Kind::LocalControlBootstrap => {
            value.seq == 0
                && no_data
                && value.status == 0
                && value.ttl_ms == 0
                && value.bootstrap_token.len() == 64
                && value.bootstrap_token.bytes().all(|b| b.is_ascii_hexdigit())
        }
        Kind::LocalControlReady => {
            value.seq == 0 && no_data && no_auth && value.status == 0 && value.ttl_ms == 0
        }
        Kind::LocalControlHeartbeat | Kind::LocalControlShutdown => {
            value.seq > 0 && no_data && no_auth && value.status == 0 && value.ttl_ms == 0
        }
        Kind::LocalControlRequest => {
            value.seq > 0
                && no_auth
                && value.status == 0
                && !value.request_id.is_empty()
                && value.request_id.len() <= 128
                && matches!(value.method.as_str(), "GET" | "POST")
                && value.path.starts_with("/api/v1/")
                && value.path.len() <= 2048
                && !value.path.contains(['?', '#', '\r', '\n'])
                && (1..=CALL_TIMEOUT.as_millis() as u32).contains(&value.ttl_ms)
        }
        Kind::LocalControlResponse => {
            value.seq > 0
                && no_auth
                && no_route
                && value.ttl_ms == 0
                && !value.request_id.is_empty()
                && value.request_id.len() <= 128
                && (100..=599).contains(&value.status)
        }
        Kind::LocalControlUnspecified => false,
    };
    if valid {
        Ok(kind)
    } else {
        Err("IPC_FRAME_SHAPE".into())
    }
}

struct Job {
    value: Frame,
    deadline: Instant,
    reply: SyncSender<Result<Frame, String>>,
}

/// Sole process owner. Blocking I/O stays in transport threads; callers retain
/// an independent kill handle and a deadline, including when a pipe is wedged.
pub struct ControlProcess {
    jobs: SyncSender<Job>,
    child: Arc<Mutex<Child>>,
    failed: Arc<AtomicBool>,
    closed: AtomicBool,
}

fn terminate(child: &Mutex<Child>, failed: &AtomicBool) {
    failed.store(true, Ordering::Release);
    if let Ok(mut child) = child.lock() {
        let _ = child.kill();
        let _ = child.try_wait();
    }
}

impl ControlProcess {
    /// Path and args come from trusted native composition, never a WebView.
    pub fn spawn(command: &mut Command) -> Result<Self, String> {
        let mut random = [0; 32];
        getrandom::fill(&mut random).map_err(|_| "IPC_RANDOM_UNAVAILABLE")?;
        let token: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|_| "ENGINE_SPAWN_FAILED")?;
        let mut input = child.stdin.take().ok_or("IPC_STDIN_MISSING")?;
        let mut output = child.stdout.take().ok_or("IPC_STDOUT_MISSING")?;
        let child = Arc::new(Mutex::new(child));
        let failed = Arc::new(AtomicBool::new(false));
        let (received, incoming) = mpsc::sync_channel(1);
        thread::spawn(move || {
            loop {
                let next =
                    read_frame(&mut output).and_then(|item| item.ok_or_else(|| "IPC_EOF".into()));
                let end = next.is_err();
                if received.send(next).is_err() || end {
                    break;
                }
            }
        });
        let mut bootstrap = frame(Kind::LocalControlBootstrap);
        bootstrap.bootstrap_token = token;
        let ready = write_frame(&mut input, &bootstrap).and_then(|()| {
            incoming
                .recv_timeout(START_TIMEOUT)
                .map_err(|_| "ENGINE_READY_TIMEOUT".to_string())?
        });
        if !matches!(
            ready
                .as_ref()
                .map_err(|_| ())
                .and_then(|value| validate(value).map_err(|_| ())),
            Ok(Kind::LocalControlReady)
        ) {
            terminate(&child, &failed);
            return Err("ENGINE_HANDSHAKE_FAILED".into());
        }
        let (jobs, queue) = mpsc::sync_channel::<Job>(QUEUE_LIMIT);
        let owner = child.clone();
        let broken = failed.clone();
        thread::spawn(move || {
            if actor(&mut input, incoming, queue, &broken).is_err() {
                terminate(&owner, &broken);
            }
        });
        Ok(Self {
            jobs,
            child,
            failed,
            closed: AtomicBool::new(false),
        })
    }

    pub fn request(&self, mut value: Frame) -> Result<Frame, String> {
        if self.closed.load(Ordering::Acquire) || self.failed.load(Ordering::Acquire) {
            return Err("ENGINE_UNAVAILABLE".into());
        }
        if value.kind != Kind::LocalControlRequest as i32 || value.seq != 0 {
            return Err("IPC_REQUEST_REQUIRED".into());
        }
        value.seq = 1; // wire sequence belongs to the actor, not the WebView.
        validate(&value)?;
        if value.encoded_len() > FRAME_LIMIT {
            return Err("IPC_FRAME_LIMIT".into());
        }
        self.exchange(value)
    }

    fn exchange(&self, value: Frame) -> Result<Frame, String> {
        let (reply, response) = mpsc::sync_channel(1);
        let deadline = Instant::now() + CALL_TIMEOUT;
        self.jobs
            .try_send(Job {
                value,
                deadline,
                reply,
            })
            .map_err(|_| "IPC_QUEUE_FULL_OR_CLOSED")?;
        match response.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(result) => result,
            Err(_) => {
                terminate(&self.child, &self.failed);
                Err("IPC_DEADLINE_EXCEEDED_NO_RETRY".into())
            }
        }
    }

    pub fn shutdown(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        let _ = self.exchange(frame(Kind::LocalControlShutdown));
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let exited = self
                .child
                .lock()
                .ok()
                .is_some_and(|mut child| matches!(child.try_wait(), Ok(Some(_))));
            if exited {
                return;
            }
            if Instant::now() >= deadline {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        terminate(&self.child, &self.failed);
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            if self
                .child
                .lock()
                .ok()
                .is_some_and(|mut child| matches!(child.try_wait(), Ok(Some(_))))
            {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for ControlProcess {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn actor(
    input: &mut impl Write,
    incoming: Receiver<Result<Frame, String>>,
    jobs: Receiver<Job>,
    failed: &AtomicBool,
) -> Result<(), String> {
    let mut seq = 0u64;
    loop {
        if failed.load(Ordering::Acquire) {
            return Err("ENGINE_UNAVAILABLE".into());
        }
        let job = match jobs.recv_timeout(HEARTBEAT_INTERVAL) {
            Ok(job) => Some(job),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(_) => return Ok(()),
        };
        if let Some(job) = &job
            && Instant::now() >= job.deadline
        {
            let _ = job.reply.send(Err("IPC_EXPIRED_BEFORE_DISPATCH".into()));
            continue;
        }
        seq = seq.checked_add(1).ok_or("IPC_SEQUENCE_EXHAUSTED")?;
        let mut request = job
            .as_ref()
            .map(|j| j.value.clone())
            .unwrap_or_else(|| frame(Kind::LocalControlHeartbeat));
        request.seq = seq;
        let deadline = job
            .as_ref()
            .map(|j| j.deadline)
            .unwrap_or_else(|| Instant::now() + CALL_TIMEOUT);
        let result = write_frame(input, &request)
            .and_then(|()| {
                incoming
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .map_err(|_| "IPC_RESPONSE_TIMEOUT".to_string())?
            })
            .and_then(|response| {
                let kind = validate(&response)?;
                let expected = if request.kind == Kind::LocalControlRequest as i32 {
                    Kind::LocalControlResponse
                } else {
                    Kind::try_from(request.kind).map_err(|_| "IPC_KIND_INVALID")?
                };
                if kind != expected
                    || response.seq != seq
                    || response.request_id != request.request_id
                {
                    return Err("IPC_RESPONSE_IDENTITY".into());
                }
                Ok(response)
            });
        let broken = result.is_err();
        if let Some(job) = job {
            let _ = job.reply.send(result);
        }
        if broken {
            return Err("IPC_CONNECTION_FAILED_NO_RETRY".into());
        }
        if request.kind == Kind::LocalControlShutdown as i32 {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_frames_roundtrip_and_reject_partial_prefixes_and_lengths() {
        let mut request = frame(Kind::LocalControlRequest);
        request.seq = 1;
        request.request_id = "r".into();
        request.method = "GET".into();
        request.path = "/api/v1/runtime/status".into();
        request.ttl_ms = 4000;
        assert!(validate(&request).is_ok());
        let mut encoded = Vec::new();
        write_frame(&mut encoded, &request).unwrap();
        assert_eq!(read_frame(&mut encoded.as_slice()).unwrap(), Some(request));
        assert_eq!(read_frame(&mut [].as_slice()).unwrap(), None);
        for size in 1..4 {
            assert!(read_frame(&mut [0u8; 4][..size].as_ref()).is_err());
        }
        assert_eq!(
            read_frame(&mut u32::MAX.to_be_bytes().as_slice()).unwrap_err(),
            "IPC_FRAME_LIMIT"
        );
        assert_eq!(
            read_frame(&mut [0, 0, 0, 2, 8].as_slice()).unwrap_err(),
            "IPC_TRUNCATED_BODY"
        );
    }
    #[test]
    fn credentials_and_version_cannot_be_smuggled_across_message_kinds() {
        let mut heartbeat = frame(Kind::LocalControlHeartbeat);
        heartbeat.seq = 1;
        assert!(validate(&heartbeat).is_ok());
        heartbeat.bootstrap_token = "a".repeat(64);
        assert!(validate(&heartbeat).is_err());
        heartbeat.bootstrap_token.clear();
        heartbeat.version = 99;
        assert!(validate(&heartbeat).is_err());
        let mut oversized = frame(Kind::LocalControlResponse);
        oversized.body = vec![0; FRAME_LIMIT];
        assert_eq!(
            write_frame(&mut Vec::new(), &oversized).unwrap_err(),
            "IPC_FRAME_LIMIT"
        );
    }
    #[test]
    fn a_peer_that_never_answers_hits_the_deadline_and_the_request_is_not_retried() {
        let (_tx, incoming) = mpsc::channel::<Result<Frame, String>>();
        let (jobs, queue) = mpsc::sync_channel::<Job>(1);
        let (reply, response) = mpsc::sync_channel(1);
        let mut request = frame(Kind::LocalControlRequest);
        request.seq = 1;
        request.request_id = "slow".into();
        request.method = "GET".into();
        request.path = "/api/v1/runtime/status".into();
        request.ttl_ms = 4000;
        let deadline = Instant::now() + CALL_TIMEOUT;
        jobs.send(Job {
            value: request,
            deadline,
            reply,
        })
        .unwrap();
        let started = Instant::now();
        let handle = thread::spawn(move || {
            let mut wire = Vec::new();
            let result = actor(&mut wire, incoming, queue, &AtomicBool::new(false));
            (result, wire)
        });
        assert_eq!(
            response
                .recv_timeout(CALL_TIMEOUT + Duration::from_secs(1))
                .unwrap(),
            Err("IPC_RESPONSE_TIMEOUT".into())
        );
        assert!(started.elapsed() >= CALL_TIMEOUT);
        let (result, wire) = handle.join().unwrap();
        assert_eq!(result, Err("IPC_CONNECTION_FAILED_NO_RETRY".into()));
        // One request on the wire, then no second attempt.
        let mut wire = wire.as_slice();
        assert!(read_frame(&mut wire).unwrap().is_some());
        assert!(
            read_frame(&mut wire).unwrap().is_none(),
            "a timed-out request must not be retried"
        );
    }
}
