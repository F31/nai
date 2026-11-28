//! Explicit local audition composition: streaming adapter + independent device.
use nai_contracts::workbench::WorkbenchError as Error;
use nai_llm_openai::cosyvoice::Config;
#[cfg(windows)]
use nai_runtime_core::foundation::Operation;
use std::path::Path;
#[cfg(any(windows, test))]
pub(crate) async fn wait_device_ready<T>(
    future: impl std::future::Future<Output = Result<T, Error>>,
    operation: &nai_runtime_core::foundation::Operation,
) -> Result<T, Error> {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut future = Box::pin(future);
    loop {
        operation.check()?;
        if std::time::Instant::now() >= until {
            return Err(Error::DeadlineExceeded);
        }
        match tokio::time::timeout(std::time::Duration::from_millis(10), future.as_mut()).await {
            Ok(value) => {
                operation.check()?;
                return value;
            }
            Err(_) => continue,
        }
    }
}
#[cfg(test)]
mod device_preparation_tests {
    use super::*;
    #[test]
    fn pending_device_initialization_observes_parent_cancel() {
        let parent = nai_runtime_core::foundation::Operation::model_task(1000).unwrap();
        let cancel = parent.clone();
        let started = std::time::Instant::now();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            cancel.cancel();
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime.block_on(wait_device_ready(
            std::future::pending::<Result<(), Error>>(),
            &parent,
        ));
        thread.join().unwrap();
        assert_eq!(result, Err(Error::Cancelled));
        assert!(started.elapsed() < std::time::Duration::from_millis(500));
    }
}
#[cfg(windows)]
use std::time::Instant;

#[derive(Clone, Copy)]
pub struct Options {
    pub capacity_samples: u32,
    pub prefill_samples: u32,
}
pub(crate) const DEFAULT_CAPACITY_SAMPLES: u32 = 96_000;
impl Default for Options {
    fn default() -> Self {
        Self {
            capacity_samples: DEFAULT_CAPACITY_SAMPLES,
            prefill_samples: 2400,
        }
    }
}
#[derive(Default, serde::Serialize)]
pub struct StartupMarks {
    pub first_payload_ack_ms: Option<u64>,
    pub first_authorized_ms: Option<u64>,
    pub first_submitted_ms: Option<u64>,
    pub rpc_count_before_play: u64,
    pub rpc_us_before_play: u128,
    pub paced_wait_us_before_play: u128,
}
pub struct Report {
    pub startup_marks: Option<StartupMarks>,
    pub device_process_id: u32,
    pub first_played_at: std::time::Instant,
    pub completed_at: std::time::Instant,
    pub samples: u64,
    pub played_samples: u64,
    pub first_pcm_ms: u64,
    pub first_played_ms: u64,
    pub http_eof_ms: u64,
    pub completed_ms: u64,
    pub sentences: u32,
    pub overlap_prefetches: u32,
    pub model_calls: u32,
    pub clock_observations: u64,
    pub max_clock_age_ms: u64,
    pub epoch: u64,
    pub device_generation: u64,
}

#[cfg(any(windows, test))]
fn authenticate_receipt(
    mut receipt: nai_contracts::v561_stream_audio::Receipt,
    key: &ring::hmac::Key,
    epoch: u64,
    sequence: u64,
) -> Result<nai_contracts::v561_stream_audio::Receipt, Error> {
    use prost::Message;
    let mac = std::mem::take(&mut receipt.mac);
    ring::hmac::verify(key, &receipt.encode_to_vec(), &mac)
        .map_err(|_| Error::AuthenticationFailed)?;
    if receipt.version != nai_contracts::v561_stream_audio::WireVersion::Current as u32
        || receipt.epoch != epoch
        || receipt.sequence != sequence
    {
        return Err(Error::ProtocolError);
    }
    Ok(receipt)
}

#[cfg(not(windows))]
pub fn audition(
    _config: Config,
    _text: String,
    _binary: &Path,
    _options: Options,
) -> Result<Report, Error> {
    Err(Error::CapabilityUnavailable)
}
#[cfg(windows)]
pub fn audition(
    config: Config,
    text: String,
    binary: &Path,
    options: Options,
) -> Result<Report, Error> {
    audition_with_clock(config, text, binary, options, None)
}
#[cfg(not(windows))]
pub fn audition_with_clock(
    _config: Config,
    _text: String,
    _binary: &Path,
    _options: Options,
    _clock: Option<crate::v561_stream_clock::ClockPublisher>,
) -> Result<Report, Error> {
    Err(Error::CapabilityUnavailable)
}
#[cfg(windows)]
pub fn audition_with_clock(
    config: Config,
    text: String,
    binary: &Path,
    options: Options,
    clock: Option<crate::v561_stream_clock::ClockPublisher>,
) -> Result<Report, Error> {
    let mut tasks = crate::v561_tasks::Tasks::default();
    let task = tasks.begin(
        nai_kernel::output_owner::Speaker::Narration,
        config.deadline_ms,
    )?;
    let plan = nai_runtime_core::v561_narration::Plan::new(
        &text,
        config.lookahead.clone(),
        task.operation(),
    )?;
    let result = audition_plan_with_clock(config, plan, binary, options, task.operation(), clock);
    let finish = tasks.finish(&task);
    match result {
        Ok(report) => {
            finish?;
            Ok(report)
        }
        Err(error) => Err(error),
    }
}
#[cfg(not(windows))]
pub fn audition_in_task(
    _config: Config,
    _text: String,
    _binary: &Path,
    _options: Options,
    _parent: nai_runtime_core::foundation::Operation,
) -> Result<Report, Error> {
    Err(Error::CapabilityUnavailable)
}
#[cfg(windows)]
pub fn audition_in_task(
    config: Config,
    text: String,
    binary: &Path,
    options: Options,
    parent: Operation,
) -> Result<Report, Error> {
    let plan = nai_runtime_core::v561_narration::Plan::new(
        &text,
        config.lookahead.clone(),
        parent.clone(),
    )?;
    audition_plan_in_task(config, plan, binary, options, parent)
}
#[cfg(not(windows))]
pub fn audition_plan_in_task(
    _config: Config,
    _plan: nai_runtime_core::v561_narration::Plan,
    _binary: &Path,
    _options: Options,
    _parent: nai_runtime_core::foundation::Operation,
) -> Result<Report, Error> {
    Err(Error::CapabilityUnavailable)
}
#[cfg(windows)]
pub fn audition_plan_in_task(
    config: Config,
    plan: nai_runtime_core::v561_narration::Plan,
    binary: &Path,
    options: Options,
    parent: Operation,
) -> Result<Report, Error> {
    audition_plan_with_clock(config, plan, binary, options, parent, None)
}
#[cfg(not(windows))]
pub fn audition_plan_with_clock(
    _config: Config,
    _plan: nai_runtime_core::v561_narration::Plan,
    _binary: &Path,
    _options: Options,
    _parent: nai_runtime_core::foundation::Operation,
    _clock: Option<crate::v561_stream_clock::ClockPublisher>,
) -> Result<Report, Error> {
    Err(Error::CapabilityUnavailable)
}
#[cfg(windows)]
pub fn audition_plan_with_clock(
    config: Config,
    plan: nai_runtime_core::v561_narration::Plan,
    binary: &Path,
    options: Options,
    parent: Operation,
    clock_publisher: Option<crate::v561_stream_clock::ClockPublisher>,
) -> Result<Report, Error> {
    audition_plan_with_source(config, plan, binary, options, parent, clock_publisher, None)
}
#[cfg(windows)]
pub fn audition_plan_with_source(
    config: Config,
    plan: nai_runtime_core::v561_narration::Plan,
    binary: &Path,
    options: Options,
    parent: Operation,
    clock_publisher: Option<crate::v561_stream_clock::ClockPublisher>,
    mut source: Option<&mut crate::v561_pcm_source::Source>,
) -> Result<Report, Error> {
    run_source(
        SourceInput::Narration(config, plan),
        binary,
        options,
        parent,
        clock_publisher,
        source.take(),
    )
}
#[cfg(windows)]
enum SourceInput {
    Narration(Config, nai_runtime_core::v561_narration::Plan),
    Prepared(Vec<u8>, u64, Option<std::sync::mpsc::Receiver<()>>),
    External(crate::v561_external_pcm::Source, u64),
}
/// Source already bound to the parent's cancellation; never calls a model.
#[cfg(windows)]
pub fn play_external_with_clock(
    source: crate::v561_external_pcm::Source,
    binary: &Path,
    parent: Operation,
    clock: crate::v561_stream_clock::ClockPublisher,
    epoch: u64,
    options: Options,
) -> Result<Report, Error> {
    if epoch == 0 {
        return Err(Error::InvalidArgument);
    }
    run_source(
        SourceInput::External(source, epoch),
        binary,
        options,
        parent,
        Some(clock),
        None,
    )
}
/// Plays already validated source bytes; never invokes a model or synthesizes substitutes.
#[cfg(windows)]
pub fn play_prepared_with_clock(
    bytes: Vec<u8>,
    binary: &Path,
    parent: Operation,
    clock: crate::v561_stream_clock::ClockPublisher,
    epoch: u64,
) -> Result<Report, Error> {
    prepared_source(bytes, binary, parent, clock, epoch, None)
}
/// Media stays unsubmitted until the root consumer has admitted its frame timeline.
#[cfg(windows)]
pub fn play_prepared_with_gate(
    bytes: Vec<u8>,
    binary: &Path,
    parent: Operation,
    clock: crate::v561_stream_clock::ClockPublisher,
    epoch: u64,
    ready: std::sync::mpsc::Receiver<()>,
) -> Result<Report, Error> {
    prepared_source(bytes, binary, parent, clock, epoch, Some(ready))
}
#[cfg(windows)]
fn prepared_source(
    bytes: Vec<u8>,
    binary: &Path,
    parent: Operation,
    clock: crate::v561_stream_clock::ClockPublisher,
    epoch: u64,
    ready: Option<std::sync::mpsc::Receiver<()>>,
) -> Result<Report, Error> {
    if epoch == 0 || bytes.is_empty() || bytes.len() > 288000 || !bytes.len().is_multiple_of(2) {
        return Err(Error::InvalidArgument);
    }
    run_source(
        SourceInput::Prepared(bytes, epoch, ready),
        binary,
        Options::default(),
        parent,
        Some(clock),
        None,
    )
}
#[cfg(windows)]
fn run_source(
    input: SourceInput,
    binary: &Path,
    options: Options,
    parent: Operation,
    clock_publisher: Option<crate::v561_stream_clock::ClockPublisher>,
    mut source: Option<&mut crate::v561_pcm_source::Source>,
) -> Result<Report, Error> {
    let bound_epoch = match &input {
        SourceInput::Prepared(_, epoch, _) => Some(*epoch),
        SourceInput::External(_, epoch) => Some(*epoch),
        _ => None,
    };
    let (deadline_ms, max_samples) = match &input {
        SourceInput::Narration(config, _) => (config.deadline_ms, config.max_samples),
        SourceInput::Prepared(bytes, _, _) => (45000, (bytes.len() / 2) as u64),
        SourceInput::External(source, _) => (45000, source.maximum),
    };
    if let Some(source) = source.as_deref_mut() {
        source.begin(max_samples)?;
    }
    use nai_contracts::v561_stream_audio::{
        Begin, Chunk, Command as Packet, Receipt, Renew, Seal, State, WireVersion, command::Action,
    };
    use prost::Message;
    use ring::{
        hmac,
        rand::{SecureRandom, SystemRandom},
    };
    use sha2::{Digest, Sha256};
    use std::{
        io::Write,
        os::windows::process::CommandExt,
        process::{Child, Command, Stdio},
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
            mpsc,
        },
        time::Duration,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::windows::named_pipe::{ClientOptions, NamedPipeClient},
    };
    if !binary.is_absolute()
        || binary.file_name().and_then(|s| s.to_str()) != Some("nai-v561-stream-audio-output.exe")
        || !binary.is_file()
        || options.capacity_samples < 4800
        || options.capacity_samples > 96000
        || options.prefill_samples == 0
        || options.prefill_samples > options.capacity_samples
    {
        return Err(Error::InvalidArgument);
    }
    let operation = parent.model_child(deadline_ms)?;
    let started = Instant::now();
    struct Cancel(Operation);
    impl Drop for Cancel {
        fn drop(&mut self) {
            self.0.cancel();
        }
    }
    let _cancel = Cancel(operation.clone());
    struct Reap {
        child: Child,
        data: Option<mpsc::SyncSender<Vec<u8>>>,
        writer: Option<std::thread::JoinHandle<()>>,
    }
    impl Drop for Reap {
        fn drop(&mut self) {
            self.data.take();
            let _ = self.child.kill();
            let _ = self.child.wait();
            if let Some(t) = self.writer.take() {
                let _ = t.join();
            }
        }
    }
    let mut secret = zeroize::Zeroizing::new([0u8; 64]);
    SystemRandom::new()
        .fill(&mut *secret)
        .map_err(|_| Error::IoFailed)?;
    let hex = |data: &[u8]| data.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let pipe_name = format!(r"\\.\pipe\nai-v561-stream-{}", hex(&secret[..32]));
    let epoch = bound_epoch
        .unwrap_or(u64::from_le_bytes(secret[..8].try_into().map_err(|_| Error::IoFailed)?) | 1);
    let key = hmac::Key::new(hmac::HMAC_SHA256, &secret[32..]);
    let mut command = Command::new(binary);
    command.env_clear();
    for name in ["SystemRoot", "WINDIR"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let child = command
        .arg("--pipe")
        .arg(&pipe_name)
        .env("NAI_V561_AUDIO_TOKEN", hex(&secret[32..]))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|_| Error::IoFailed)?;
    let mut reap = Reap {
        child,
        data: None,
        writer: None,
    };
    let mut stdin = reap.child.stdin.take().ok_or(Error::IoFailed)?;
    let (write_tx, write_rx) = mpsc::sync_channel::<Vec<u8>>(1);
    reap.data = Some(write_tx);
    reap.writer = Some(std::thread::spawn(move || {
        while let Ok(data) = write_rx.recv() {
            if stdin
                .write_all(&(data.len() as u32).to_le_bytes())
                .and_then(|_| stdin.write_all(&data))
                .and_then(|_| stdin.flush())
                .is_err()
            {
                break;
            }
        }
    }));
    async fn exchange(
        pipe: &mut NamedPipeClient,
        key: &hmac::Key,
        epoch: u64,
        sequence: u64,
        action: Action,
    ) -> Result<(Receipt, Instant), Error> {
        let request_started = Instant::now();
        let mut packet = Packet {
            version: WireVersion::Current as u32,
            epoch,
            sequence,
            mac: vec![],
            action: Some(action),
        };
        packet.mac = hmac::sign(key, &packet.encode_to_vec()).as_ref().to_vec();
        let bytes = packet.encode_to_vec();
        tokio::time::timeout(Duration::from_millis(200), async {
            pipe.write_u32_le(bytes.len() as u32)
                .await
                .map_err(|_| Error::IoFailed)?;
            pipe.write_all(&bytes).await.map_err(|_| Error::IoFailed)?;
            pipe.flush().await.map_err(|_| Error::IoFailed)?;
            let n = pipe.read_u32_le().await.map_err(|_| Error::IoFailed)? as usize;
            if n == 0 || n > 1024 {
                return Err(Error::ProtocolError);
            }
            let mut bytes = vec![0; n];
            pipe.read_exact(&mut bytes)
                .await
                .map_err(|_| Error::IoFailed)?;
            let receipt = authenticate_receipt(
                Receipt::decode(bytes.as_slice()).map_err(|_| Error::ProtocolError)?,
                key,
                epoch,
                sequence,
            )?;
            if receipt.version != WireVersion::Current as u32
                || receipt.epoch != epoch
                || receipt.sequence != sequence
                || receipt.played_samples > receipt.submitted_samples
                || receipt.submitted_samples > receipt.received_samples
                || State::try_from(receipt.state)
                    .ok()
                    .filter(|s| *s != State::Unspecified)
                    .is_none()
            {
                return Err(Error::ProtocolError);
            }
            Ok((receipt, request_started))
        })
        .await
        .map_err(|_| Error::DeadlineExceeded)?
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Error::IoFailed)?;
    let result = runtime.block_on(async {
        let connect_until = Instant::now() + Duration::from_secs(3);
        let mut pipe = loop {
            operation.check()?;
            match ClientOptions::new().open(&pipe_name) {
                Ok(pipe) => break pipe,
                Err(_) => {
                    if Instant::now() >= connect_until {
                        return Err(Error::IoFailed);
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        };
        let begin = Begin {
            sample_rate: 24000,
            capacity_samples: options.capacity_samples,
            max_samples,
            prefill_samples: options.prefill_samples,
        };
        // Device initialization is silent and must not consume an active lease.
        let mut packet = Packet {
            version: WireVersion::Current as u32,
            epoch,
            sequence: 1,
            mac: vec![],
            action: Some(Action::Begin(begin)),
        };
        packet.mac = hmac::sign(&key, &packet.encode_to_vec()).as_ref().to_vec();
        let bytes = packet.encode_to_vec();
        pipe.write_u32_le(bytes.len() as u32)
            .await
            .map_err(|_| Error::IoFailed)?;
        pipe.write_all(&bytes).await.map_err(|_| Error::IoFailed)?;
        let mut receipt = wait_device_ready(
            async {
                let n = pipe.read_u32_le().await.map_err(|_| Error::IoFailed)? as usize;
                if n == 0 || n > 1024 {
                    return Err(Error::ProtocolError);
                }
                let mut bytes = vec![0; n];
                pipe.read_exact(&mut bytes)
                    .await
                    .map_err(|_| Error::IoFailed)?;
                let r = authenticate_receipt(
                    Receipt::decode(bytes.as_slice()).map_err(|_| Error::ProtocolError)?,
                    &key,
                    epoch,
                    1,
                )?;
                if r.version != WireVersion::Current as u32
                    || r.epoch != epoch
                    || r.sequence != 1
                    || r.state != State::Prepared as i32
                    || r.received_samples != 0
                    || r.played_samples != 0
                    || r.free_samples != options.capacity_samples
                {
                    return Err(Error::ProtocolError);
                }
                Ok(r)
            },
            &operation,
        )
        .await?;
        enum Event {
            Data(Vec<u8>),
            End(Result<crate::v561_narration::Production, Error>),
        }
        let (tx, rx) = mpsc::sync_channel::<Event>(2);
        let first_pcm = Arc::new(AtomicU64::new(0));
        let http_eof = Arc::new(AtomicU64::new(0));
        let played = Arc::new(AtomicU64::new(0));
        let source_played = played.clone();
        let (first, eof, worker_op) = (first_pcm.clone(), http_eof.clone(), operation.clone());
        std::thread::spawn(move || {
            let send = |mut event: Event| -> Result<(), Error> {
                loop {
                    worker_op.check()?;
                    match tx.try_send(event) {
                        Ok(()) => return Ok(()),
                        Err(mpsc::TrySendError::Full(value)) => {
                            event = value;
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => return Err(Error::Cancelled),
                    }
                }
            };
            let mut push = |data: &[u8]| {
                first
                    .compare_exchange(
                        0,
                        started.elapsed().as_millis() as u64 + 1,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .ok();
                send(Event::Data(data.to_vec()))
            };
            let result = match input {
                SourceInput::External(source, _) => {
                    source
                        .consume(&mut push)
                        .map(|samples| crate::v561_narration::Production {
                            samples,
                            transport_eof_at: Instant::now(),
                            sentences: 1,
                            overlap_prefetches: 0,
                            model_calls: 0,
                        })
                }
                SourceInput::Narration(config, plan) => crate::v561_narration::produce(
                    config,
                    plan,
                    worker_op.clone(),
                    source_played,
                    &mut push,
                ),
                SourceInput::Prepared(bytes, _, ready) => (|| {
                    if let Some(ready) = ready {
                        loop {
                            worker_op.check()?;
                            match ready.recv_timeout(Duration::from_millis(10)) {
                                Ok(()) => break,
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                                Err(_) => return Err(Error::Cancelled),
                            }
                        }
                    }
                    for part in bytes.chunks(9600) {
                        push(part)?;
                    }
                    Ok(crate::v561_narration::Production {
                        samples: (bytes.len() / 2) as u64,
                        transport_eof_at: Instant::now(),
                        sentences: 1,
                        overlap_prefetches: 0,
                        model_calls: 0,
                    })
                })(),
            };
            if let Ok(summary) = &result {
                // Keep the transport metric distinct from the later completion receipt.
                eof.store(
                    summary
                        .transport_eof_at
                        .saturating_duration_since(started)
                        .as_millis() as u64
                        + 1,
                    Ordering::Release,
                );
            }
            let _ = send(Event::End(result));
        });
        let mut sequence = 1;
        let mut request_at = Instant::now();
        let mut clock_bridge = nai_media_core::stream_clock::StreamClock::new(epoch, max_samples)
            .ok_or(Error::InvalidArgument)?;
        let mut clock_observations = 0u64;
        let mut max_clock_age_ms = 0u64;
        let mut chunk_sequence = 0;
        let mut sent = 0u64;
        let mut hash = Sha256::new();
        let mut pending = None;
        let mut end: Option<crate::v561_narration::Production> = None;
        let mut sealed = false;
        let mut first_played = None;
        let mut first_played_at = None;
        let mut previous_played = 0;
        let mut renewed = Instant::now() - Duration::from_millis(100);
        loop {
            operation.check()?;
            let (clock, deadline) = clock_bridge
                .accept(&receipt, request_at)
                .map_err(|_| Error::ProtocolError)?;
            clock_observations += 1;
            max_clock_age_ms = max_clock_age_ms.max(
                200u64.saturating_sub(
                    deadline
                        .saturating_duration_since(Instant::now())
                        .as_millis() as u64,
                ),
            );
            if let Some(publisher) = &clock_publisher {
                publisher
                    .publish(clock, deadline, operation.clone())
                    .inspect_err(|_| {
                        eprintln!("NAI_STREAM_CLOCK_HANDOFF_FAILED");
                    })?;
            }
            if receipt.state == State::Stopped as i32 {
                return Err(Error::Cancelled);
            }
            if receipt.free_samples > options.capacity_samples
                || receipt.received_samples != sent
                || receipt.played_samples < previous_played
            {
                return Err(Error::ProtocolError);
            }
            previous_played = receipt.played_samples;
            played.store(receipt.played_samples, Ordering::Release);
            if receipt.played_samples > 0 && first_played.is_none() {
                let at = Instant::now();
                first_played = Some(at.saturating_duration_since(started).as_millis() as u64);
                first_played_at = Some(at);
            }
            if receipt.state == State::Completed as i32 {
                if !sealed
                    || receipt.played_samples != sent
                    || !receipt.sealed
                    || http_eof.load(Ordering::Acquire) == 0
                {
                    return Err(Error::ProtocolError);
                }
                pipe.write_u8(0).await.map_err(|_| Error::IoFailed)?;
                if let Some(source) = source.as_deref_mut() {
                    source.seal(epoch, sent, &hash.clone().finalize())?;
                }
                return Ok(Report {
                    startup_marks: None,
                    device_process_id: reap.child.id(),
                    first_played_at: first_played_at.ok_or(Error::ProtocolError)?,
                    completed_at: Instant::now(),
                    samples: sent,
                    played_samples: receipt.played_samples,
                    first_pcm_ms: first_pcm.load(Ordering::Acquire).saturating_sub(1),
                    first_played_ms: first_played.ok_or(Error::ProtocolError)?,
                    http_eof_ms: http_eof.load(Ordering::Acquire) - 1,
                    completed_ms: started.elapsed().as_millis() as u64,
                    sentences: end.as_ref().ok_or(Error::ProtocolError)?.sentences,
                    overlap_prefetches: end
                        .as_ref()
                        .ok_or(Error::ProtocolError)?
                        .overlap_prefetches,
                    model_calls: end.as_ref().ok_or(Error::ProtocolError)?.model_calls,
                    clock_observations,
                    max_clock_age_ms,
                    epoch,
                    device_generation: clock.device_generation,
                });
            }
            if pending.is_none() && end.is_none() {
                match rx.try_recv() {
                    Ok(Event::Data(data)) => pending = Some(data),
                    Ok(Event::End(result)) => end = Some(result?),
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(_) => return Err(Error::IoFailed),
                }
            }
            let action = if renewed.elapsed() >= Duration::from_millis(50) {
                renewed = Instant::now();
                Action::Renew(Renew {
                    ttl_ms: 300,
                    play_until_sample: sent,
                })
            } else if pending
                .as_ref()
                .is_some_and(|d| d.len() / 2 <= receipt.free_samples as usize)
            {
                let data = pending.take().ok_or(Error::ProtocolError)?;
                let count = (data.len() / 2) as u32;
                chunk_sequence += 1;
                let meta = Chunk {
                    chunk_sequence,
                    source_sample: sent,
                    samples: count,
                    sha256: Sha256::digest(&data).to_vec(),
                };
                if let Some(source) = source.as_deref_mut() {
                    source.append(&data)?;
                }
                hash.update(&data);
                sent += u64::from(count);
                reap.data
                    .as_ref()
                    .ok_or(Error::IoFailed)?
                    .try_send(data)
                    .map_err(|_| {
                        eprintln!("NAI_STREAM_PCM_HANDOFF_FAILED");
                        Error::ResourceExhausted
                    })?;
                Action::Chunk(meta)
            } else if let Some(summary) = end.as_ref().filter(|_| !sealed) {
                if summary.samples != sent {
                    return Err(Error::ProtocolError);
                }
                sealed = true;
                Action::Seal(Seal {
                    total_samples: sent,
                    sha256: hash.clone().finalize().to_vec(),
                })
            } else {
                renewed = Instant::now();
                Action::Renew(Renew {
                    ttl_ms: 300,
                    play_until_sample: sent,
                })
            };
            sequence += 1;
            (receipt, request_at) = exchange(&mut pipe, &key, epoch, sequence, action).await?;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
    operation.cancel();
    result
}

#[cfg(test)]
mod clock_auth_tests {
    use super::*;
    use nai_contracts::{
        v561_media::{ClockState, PlaybackClockObservation},
        v561_stream_audio::{Receipt, WireVersion},
    };
    use prost::Message;
    use ring::hmac;
    #[test]
    fn nested_clock_and_device_age_are_inside_transport_signature() {
        let key = hmac::Key::new(hmac::HMAC_SHA256, &[17; 32]);
        let mut r = Receipt {
            version: WireVersion::Current as u32,
            epoch: 7,
            sequence: 2,
            clock: Some(PlaybackClockObservation {
                epoch: 7,
                device_generation: 1,
                observation_sequence: Some(2),
                state: ClockState::Running as i32,
                played_sample: Some(20),
                sample_rate: 24000,
            }),
            ..Default::default()
        };
        r.mac = hmac::sign(&key, &r.encode_to_vec()).as_ref().to_vec();
        assert!(authenticate_receipt(r.clone(), &key, 7, 2).is_ok());
        for fault in 0..5 {
            let mut changed = r.clone();
            match fault {
                0 => changed.clock_age_ms = 1,
                1 => changed.clock.as_mut().unwrap().played_sample = Some(21),
                2 => changed.clock.as_mut().unwrap().device_generation = 2,
                3 => changed.clock = None,
                _ => changed.segments.push(Default::default()),
            }
            assert!(matches!(
                authenticate_receipt(changed, &key, 7, 2),
                Err(Error::AuthenticationFailed)
            ));
        }
    }
}
