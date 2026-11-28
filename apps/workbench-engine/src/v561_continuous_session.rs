//! One authenticated audio child for an entire script. No model or frame I/O.
use crate::{
    v561_external_pcm::Source, v561_stream_audio::Report, v561_stream_clock::ClockPublisher,
};
use nai_contracts::{
    v561_continuous_audio::{OpenScript, SegmentBegin, SegmentIdentity, SegmentSeal},
    v561_media::{ClockState, PlaybackClockObservation},
    v561_stream_audio::{
        Chunk, Command, Receipt, Renew, Seal, State, WireVersion, command::Action,
    },
    workbench::WorkbenchError as Error,
};
use nai_runtime_core::foundation::Operation;
use prost::Message;
use ring::{
    hmac,
    rand::{SecureRandom, SystemRandom},
};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeClient},
};

enum Request {
    Segment(
        Source,
        u64,
        Operation,
        ClockPublisher,
        mpsc::SyncSender<Result<Report, Error>>,
    ),
    Finish(mpsc::SyncSender<Result<(), Error>>),
}
pub(crate) const MAX_SEGMENTS: u32 = 32;
pub(crate) const MAX_SAMPLES: u64 = 480000;
#[derive(Clone)]
pub struct Handle {
    tx: mpsc::SyncSender<Request>,
    root: Operation,
}
pub struct Session {
    handle: Handle,
    worker: Option<std::thread::JoinHandle<()>>,
    pub pid: u32,
    pub startup: Startup,
}
#[derive(Clone, Default, serde::Serialize)]
pub struct Startup {
    parent_wait_us: u128,
    spawn_us: u128,
    pipe_connect_us: u128,
    device_open_us: u128,
    renew_us: u128,
    total_us: u128,
}
fn receive<T>(rx: &mpsc::Receiver<Result<T, Error>>, op: &Operation) -> Result<T, Error> {
    loop {
        op.check()?;
        match rx.recv_timeout(Duration::from_millis(10)) {
            Ok(value) => return value,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return Err(Error::OutcomeUnknown),
        }
    }
}
fn sentence_clock(
    mut clock: PlaybackClockObservation,
    id: SegmentIdentity,
    received: u64,
) -> Result<PlaybackClockObservation, Error> {
    let played = clock.played_sample.ok_or(Error::ProtocolError)?;
    let end = id
        .source_start_sample
        .checked_add(received)
        .ok_or(Error::ProtocolError)?;
    if clock.epoch != id.script_epoch
        || id.generation_epoch == 0
        || clock.device_generation == 0
        || clock.state != ClockState::Running as i32
        || clock.sample_rate != 24000
        || played < id.source_start_sample
        || played > end
    {
        return Err(Error::ProtocolError);
    }
    clock.epoch = id.generation_epoch;
    clock.played_sample = Some(played - id.source_start_sample);
    Ok(clock)
}
impl Session {
    pub fn start(binary: &Path, root: Operation, sentences: u32) -> Result<Self, Error> {
        if !(1..=MAX_SEGMENTS).contains(&sentences) {
            return Err(Error::InvalidArgument);
        }
        Self::start_bounded(binary, root, sentences, MAX_SAMPLES)
    }

    pub(crate) fn start_bounded(
        binary: &Path,
        root: Operation,
        sentences: u32,
        maximum: u64,
    ) -> Result<Self, Error> {
        let parent_began = Instant::now();
        if !binary.is_absolute()
            || binary.file_name().and_then(|n| n.to_str())
                != Some("nai-v561-stream-audio-output.exe")
            || !binary.is_file()
            || !(1..=nai_media_core::continuous_audio::MAX_SCRIPT_SEGMENTS).contains(&sentences)
            || !(1..=nai_media_core::continuous_audio::MAX_SCRIPT_SAMPLES).contains(&maximum)
        {
            return Err(Error::InvalidArgument);
        }
        let (tx, rx) = mpsc::sync_channel(1);
        let (ready, started) = mpsc::sync_channel(1);
        let operation = root.clone();
        let binary = binary.to_owned();
        let worker = std::thread::spawn(move || {
            let result = (|| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|_| Error::IoFailed)?;
                runtime.block_on(async {
                    let mut link = Link::open(binary, &operation, sentences, maximum).await?;
                    ready
                        .send(Ok((link.child.id(), link.startup.clone())))
                        .map_err(|_| Error::Cancelled)?;
                    loop {
                        operation.check()?;
                        match rx.try_recv() {
                            Ok(Request::Segment(source, epoch, op, clock, response)) => {
                                let result = link.segment(source, epoch, op.clone(), clock).await;
                                let failed = result.is_err();
                                let _ = response.send(result);
                                if failed {
                                    op.cancel();
                                    return Err(Error::Cancelled);
                                }
                            }
                            Ok(Request::Finish(response)) => {
                                let result = link.finish().await;
                                let _ = response.send(result);
                                return Ok(());
                            }
                            Err(mpsc::TryRecvError::Disconnected) => return Err(Error::Cancelled),
                            Err(mpsc::TryRecvError::Empty) => {
                                link.exchange(Action::Renew(Renew {
                                    ttl_ms: 300,
                                    play_until_sample: link.sent,
                                }))
                                .await?;
                                tokio::time::sleep(Duration::from_millis(20)).await;
                            }
                        }
                    }
                })
            })();
            if let Err(e) = result {
                let _ = ready.try_send(Err(e));
                operation.cancel();
            }
        });
        let (pid, mut startup) = match receive(&started, &root) {
            Ok(value) => value,
            Err(e) => {
                root.cancel();
                let _ = worker.join();
                return Err(e);
            }
        };
        startup.parent_wait_us = parent_began.elapsed().as_micros();
        Ok(Self {
            handle: Handle { tx, root },
            worker: Some(worker),
            pid,
            startup,
        })
    }
    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }
    pub fn finish(&mut self) -> Result<(), Error> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.handle
            .tx
            .try_send(Request::Finish(tx))
            .map_err(|_| Error::ResourceExhausted)?;
        receive(&rx, &self.handle.root)?;
        if let Some(t) = self.worker.take() {
            t.join().map_err(|_| Error::OutcomeUnknown)?;
        }
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(t) = self.worker.take() {
            self.handle.root.cancel();
            let _ = t.join();
        }
    }
}
impl Handle {
    pub fn play(
        &self,
        source: Source,
        epoch: u64,
        op: Operation,
        clock: ClockPublisher,
    ) -> Result<Report, Error> {
        self.root.check()?;
        op.check()?;
        let (tx, rx) = mpsc::sync_channel(1);
        self.tx
            .try_send(Request::Segment(source, epoch, op.clone(), clock, tx))
            .map_err(|_| Error::ResourceExhausted)?;
        receive(&rx, &op)
    }
}
struct Link {
    child: Child,
    writer: Option<std::thread::JoinHandle<()>>,
    raw: Option<mpsc::SyncSender<zeroize::Zeroizing<Vec<u8>>>>,
    pipe: NamedPipeClient,
    key: hmac::Key,
    epoch: u64,
    sequence: u64,
    receipt: Receipt,
    request_at: Instant,
    bridge: nai_media_core::stream_clock::StreamClock,
    root: Operation,
    sent: u64,
    maximum: u64,
    chunk: u64,
    hash: Sha256,
    sentence: u32,
    last_generation: u64,
    validated: Option<(PlaybackClockObservation, Instant)>,
    startup: Startup,
}
impl Drop for Link {
    fn drop(&mut self) {
        self.raw.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(t) = self.writer.take() {
            let _ = t.join();
        }
    }
}
impl Link {
    async fn open(
        binary: PathBuf,
        root: &Operation,
        sentences: u32,
        maximum: u64,
    ) -> Result<Self, Error> {
        let began = Instant::now();
        let mut secret = zeroize::Zeroizing::new([0u8; 64]);
        SystemRandom::new()
            .fill(&mut *secret)
            .map_err(|_| Error::IoFailed)?;
        let hex = |b: &[u8]| b.iter().map(|v| format!("{v:02x}")).collect::<String>();
        let name = format!(r"\\.\pipe\nai-v561-stream-{}", hex(&secret[..32]));
        let epoch = u64::from_le_bytes(secret[..8].try_into().map_err(|_| Error::IoFailed)?) | 1;
        let mut command = std::process::Command::new(binary);
        command.env_clear();
        for n in ["SystemRoot", "WINDIR"] {
            if let Some(v) = std::env::var_os(n) {
                command.env(n, v);
            }
        }
        let spawn_began = Instant::now();
        let mut child = command
            .args(["--pipe", &name])
            .env("NAI_V561_AUDIO_TOKEN", hex(&secret[32..]))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|_| Error::IoFailed)?;
        let spawn_us = spawn_began.elapsed().as_micros();
        // Guard exists before any fallible connect, including cancellation.
        struct Guard(Option<Child>);
        impl Drop for Guard {
            fn drop(&mut self) {
                if let Some(c) = self.0.as_mut() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
            }
        }
        let input = child.stdin.take().ok_or(Error::IoFailed)?;
        let mut guard = Guard(Some(child));
        let pipe_began = Instant::now();
        let until = Instant::now() + Duration::from_secs(3);
        let pipe = loop {
            root.check()?;
            match ClientOptions::new().open(&name) {
                Ok(p) => break p,
                Err(_) => {
                    if Instant::now() >= until {
                        return Err(Error::DeadlineExceeded);
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        };
        let pipe_connect_us = pipe_began.elapsed().as_micros();
        let (raw, rx) = mpsc::sync_channel::<zeroize::Zeroizing<Vec<u8>>>(1);
        let writer = std::thread::spawn(move || {
            let mut input = input;
            while let Ok(mut bytes) = rx.recv() {
                let result = input
                    .write_all(&(bytes.len() as u32).to_le_bytes())
                    .and_then(|_| input.write_all(&bytes))
                    .and_then(|_| input.flush());
                zeroize::Zeroize::zeroize(&mut bytes);
                if result.is_err() {
                    break;
                }
            }
        });
        let mut link = Self {
            child: guard.0.take().ok_or(Error::IoFailed)?,
            writer: Some(writer),
            raw: Some(raw),
            pipe,
            key: hmac::Key::new(hmac::HMAC_SHA256, &secret[32..]),
            epoch,
            sequence: 0,
            receipt: Receipt::default(),
            request_at: Instant::now(),
            bridge: nai_media_core::stream_clock::StreamClock::new(epoch, maximum)
                .ok_or(Error::InvalidArgument)?,
            root: root.clone(),
            sent: 0,
            maximum,
            chunk: 0,
            hash: Sha256::new(),
            sentence: 0,
            last_generation: 0,
            validated: None,
            startup: Startup {
                spawn_us,
                pipe_connect_us,
                ..Startup::default()
            },
        };
        let open_began = Instant::now();
        link.exchange(Action::OpenScript(OpenScript {
            sample_rate: 24000,
            capacity_samples: 96000,
            max_samples: maximum,
            prefill_samples: 2400,
            max_sentences: sentences,
        }))
        .await?;
        link.startup.device_open_us = open_began.elapsed().as_micros();
        let renew_began = Instant::now();
        link.exchange(Action::Renew(Renew {
            ttl_ms: 300,
            play_until_sample: 0,
        }))
        .await?;
        link.startup.renew_us = renew_began.elapsed().as_micros();
        link.startup.total_us = began.elapsed().as_micros();
        Ok(link)
    }
    async fn exchange(
        &mut self,
        action: Action,
    ) -> Result<(PlaybackClockObservation, Instant), Error> {
        self.root.check()?;
        self.sequence = self.sequence.checked_add(1).ok_or(Error::ProtocolError)?;
        let initial = self.sequence == 1;
        let mut command = Command {
            version: WireVersion::Current as u32,
            epoch: self.epoch,
            sequence: self.sequence,
            mac: vec![],
            action: Some(action),
        };
        command.mac = hmac::sign(&self.key, &command.encode_to_vec())
            .as_ref()
            .to_vec();
        let bytes = command.encode_to_vec();
        self.request_at = Instant::now();
        let future = async {
            self.pipe
                .write_u32_le(bytes.len() as u32)
                .await
                .map_err(|_| Error::IoFailed)?;
            self.pipe
                .write_all(&bytes)
                .await
                .map_err(|_| Error::IoFailed)?;
            self.pipe.flush().await.map_err(|_| Error::IoFailed)?;
            let n = self.pipe.read_u32_le().await.map_err(|_| Error::IoFailed)? as usize;
            if n == 0 || n > 1024 {
                return Err(Error::ProtocolError);
            }
            let mut bytes = vec![0; n];
            self.pipe
                .read_exact(&mut bytes)
                .await
                .map_err(|_| Error::IoFailed)?;
            Receipt::decode(bytes.as_slice()).map_err(|_| Error::ProtocolError)
        };
        let mut receipt = if initial {
            crate::v561_stream_audio::wait_device_ready(future, &self.root).await?
        } else {
            tokio::time::timeout(Duration::from_millis(200), future)
                .await
                .map_err(|_| Error::DeadlineExceeded)??
        };
        let mac = std::mem::take(&mut receipt.mac);
        hmac::verify(&self.key, &receipt.encode_to_vec(), &mac)
            .map_err(|_| Error::AuthenticationFailed)?;
        receipt.mac = mac;
        if receipt.epoch != self.epoch
            || receipt.sequence != self.sequence
            || receipt.segments.len() > 2
            || receipt.free_samples > 96000
            || receipt.received_samples != self.sent
            || receipt.played_samples < self.receipt.played_samples
        {
            return Err(Error::ProtocolError);
        }
        let clock = self
            .bridge
            .accept(&receipt, self.request_at)
            .map_err(|_| Error::ProtocolError)?;
        self.root.check()?;
        self.receipt = receipt;
        self.validated = Some(clock);
        Ok(clock)
    }
    async fn finish(&mut self) -> Result<(), Error> {
        self.exchange(Action::Seal(Seal {
            total_samples: self.sent,
            sha256: self.hash.clone().finalize().to_vec(),
        }))
        .await?;
        if self.receipt.state != State::Completed as i32 || self.receipt.played_samples != self.sent
        {
            return Err(Error::ProtocolError);
        }
        self.pipe.write_u8(0).await.map_err(|_| Error::IoFailed)?;
        Ok(())
    }
    async fn segment(
        &mut self,
        source: Source,
        epoch: u64,
        op: Operation,
        publisher: ClockPublisher,
    ) -> Result<Report, Error> {
        if epoch <= self.last_generation {
            return Err(Error::ProtocolError);
        }
        self.sentence += 1;
        self.last_generation = epoch;
        let start = self.sent;
        let maximum = source.maximum;
        let id = SegmentIdentity {
            script_epoch: self.epoch,
            sentence_id: self.sentence,
            generation_epoch: epoch,
            source_start_sample: start,
        };
        let (_, deadline) = self
            .exchange(Action::SegmentBegin(SegmentBegin {
                identity: Some(id),
                max_samples: maximum,
            }))
            .await?;
        let generation = self
            .receipt
            .clock
            .ok_or(Error::ProtocolError)?
            .device_generation;
        publisher.publish(
            PlaybackClockObservation {
                epoch,
                device_generation: generation,
                observation_sequence: Some(1),
                state: ClockState::Unavailable as i32,
                played_sample: None,
                sample_rate: 0,
            },
            deadline,
            op.clone(),
        )?;
        enum Input {
            Data(zeroize::Zeroizing<Vec<u8>>),
            End(Result<u64, Error>),
        }
        let (tx, rx) = mpsc::sync_channel(2);
        let producer_op = op.clone();
        let producer = std::thread::spawn(move || {
            let send = |mut value: Input| -> Result<(), Error> {
                loop {
                    producer_op.check()?;
                    match tx.try_send(value) {
                        Ok(()) => return Ok(()),
                        Err(mpsc::TrySendError::Full(v)) => value = v,
                        Err(_) => return Err(Error::Cancelled),
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            };
            let result =
                source.consume(|bytes| send(Input::Data(zeroize::Zeroizing::new(bytes.to_vec()))));
            let _ = send(Input::End(result));
        });
        let began = Instant::now();
        let mut first_pcm = None;
        let mut first_played = None;
        let mut source_end = None;
        let mut pending = None;
        let mut sealed = false;
        let mut digest = Sha256::new();
        let mut observations = 0;
        let mut max_age = 0;
        let mut renewed = Instant::now();
        let mut marks = crate::v561_stream_audio::StartupMarks::default();
        let result = async {
            loop {
                op.check()?;
                self.root.check()?;
                let local = self.sent - start;
                let (mut clock, deadline) = self.validated.ok_or(Error::ProtocolError)?;
                if Instant::now() > deadline {
                    return Err(Error::DeadlineExceeded);
                }
                clock = sentence_clock(clock, id, local)?;
                observations += 1;
                max_age = max_age.max(
                    200u64.saturating_sub(
                        deadline
                            .saturating_duration_since(Instant::now())
                            .as_millis() as u64,
                    ),
                );
                let played = clock.played_sample.unwrap_or(0);
                if played > 0 {
                    first_played.get_or_insert_with(Instant::now);
                }
                if self.receipt.submitted_samples > start {
                    marks
                        .first_submitted_ms
                        .get_or_insert(began.elapsed().as_millis() as u64);
                }
                let mut completed = false;
                for p in &self.receipt.segments {
                    if p.identity != Some(id)
                        || p.received_samples != local
                        || p.played_samples != played
                        || p.submitted_samples > p.received_samples
                        || (p.completed && (!sealed || !p.sealed || played != local))
                    {
                        return Err(Error::ProtocolError);
                    }
                    completed |= p.completed;
                }
                if completed {
                    if source_end != Some(local) || local == 0 {
                        return Err(Error::ProtocolError);
                    }
                    clock.state = ClockState::Stopped as i32;
                    publisher.publish(clock, deadline, op.clone())?;
                    let first = first_played.ok_or(Error::ProtocolError)?;
                    return Ok(Report {
                        startup_marks: Some(marks),
                        device_process_id: self.child.id(),
                        first_played_at: first,
                        completed_at: Instant::now(),
                        samples: local,
                        played_samples: played,
                        first_pcm_ms: first_pcm.unwrap_or(0),
                        first_played_ms: first.saturating_duration_since(began).as_millis() as u64,
                        http_eof_ms: 0,
                        completed_ms: began.elapsed().as_millis() as u64,
                        sentences: 1,
                        overlap_prefetches: 0,
                        model_calls: 0,
                        clock_observations: observations,
                        max_clock_age_ms: max_age,
                        epoch,
                        device_generation: generation,
                    });
                }
                clock.state = ClockState::Running as i32;
                publisher.publish(clock, deadline, op.clone())?;
                if pending.is_none() && source_end.is_none() {
                    match rx.try_recv() {
                        Ok(Input::Data(bytes)) => {
                            first_pcm.get_or_insert(began.elapsed().as_millis() as u64);
                            pending = Some(bytes)
                        }
                        Ok(Input::End(r)) => source_end = Some(r?),
                        Err(mpsc::TryRecvError::Empty) => {}
                        Err(_) => return Err(Error::OutcomeUnknown),
                    }
                }
                let action = if renewed.elapsed() >= Duration::from_millis(50) {
                    renewed = Instant::now();
                    Action::Renew(Renew {
                        ttl_ms: 300,
                        play_until_sample: self.sent,
                    })
                } else if pending
                    .as_ref()
                    .is_some_and(|d| d.len() / 2 <= self.receipt.free_samples as usize)
                {
                    let bytes = pending.take().ok_or(Error::ProtocolError)?;
                    self.chunk += 1;
                    let at = self.sent;
                    self.sent = self
                        .sent
                        .checked_add(bytes.len() as u64 / 2)
                        .ok_or(Error::ProtocolError)?;
                    if self.sent > self.maximum {
                        return Err(Error::ResourceExhausted);
                    }
                    let meta = Chunk {
                        chunk_sequence: self.chunk,
                        source_sample: at,
                        samples: (bytes.len() / 2) as u32,
                        sha256: Sha256::digest(bytes.as_slice()).to_vec(),
                    };
                    digest.update(bytes.as_slice());
                    self.hash.update(bytes.as_slice());
                    self.raw
                        .as_ref()
                        .ok_or(Error::IoFailed)?
                        .try_send(bytes)
                        .map_err(|_| Error::ResourceExhausted)?;
                    Action::Chunk(meta)
                } else if let Some(count) = source_end.filter(|_| !sealed) {
                    if count != local {
                        return Err(Error::ProtocolError);
                    }
                    sealed = true;
                    Action::SegmentSeal(SegmentSeal {
                        identity: Some(id),
                        samples: count,
                        sha256: digest.clone().finalize().to_vec(),
                    })
                } else {
                    Action::Renew(Renew {
                        ttl_ms: 300,
                        play_until_sample: self.sent,
                    })
                };
                // A completed Chunk RPC already provides I/O backpressure.
                // Only idle/lease polling needs pacing; ready PCM must not pay
                // an extra timer tick for every bounded chunk.
                let poll_delay = !matches!(&action, Action::Chunk(_));
                let chunk_action = matches!(&action, Action::Chunk(_));
                let authorize_action =
                    matches!(&action,Action::Renew(r) if r.play_until_sample>start);
                let rpc_began = Instant::now();
                self.exchange(action).await?;
                if first_played.is_none() {
                    marks.rpc_count_before_play += 1;
                    marks.rpc_us_before_play += rpc_began.elapsed().as_micros();
                    if chunk_action {
                        marks
                            .first_payload_ack_ms
                            .get_or_insert(began.elapsed().as_millis() as u64);
                    }
                    if authorize_action {
                        marks
                            .first_authorized_ms
                            .get_or_insert(began.elapsed().as_millis() as u64);
                    }
                }
                if poll_delay {
                    let wait_began = Instant::now();
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    if first_played.is_none() {
                        marks.paced_wait_us_before_play += wait_began.elapsed().as_micros();
                    }
                }
            }
        }
        .await;
        if result.is_err() {
            op.cancel();
        }
        drop(rx);
        let joined = producer.join();
        if joined.is_err() {
            op.cancel();
            return Err(Error::OutcomeUnknown);
        }
        result
    }
}
#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "explicit actual device, eight silent fixture segments, no model calls or avatar acceptance"]
    fn profile_segment_transition_stages_without_models() {
        let binary =
            std::path::PathBuf::from(std::env::var_os("NAI_AUDIO_BINARY").expect("audio binary"));
        let dir = std::path::PathBuf::from(
            std::env::var_os("NAI_AUDIO_PROFILE_DIR").expect("fresh artifacts"),
        );
        assert!(!dir.exists());
        std::fs::create_dir_all(&dir).unwrap();
        let op = Operation::model_task(45000).unwrap();
        let mut session = Session::start(&binary, op.clone(), 8).unwrap();
        let handle = session.handle();
        let mut rows = Vec::new();
        let mut previous = None;
        for epoch in 1..=8 {
            let (mut tx, source) =
                crate::v561_external_pcm::channel(28800, 16, op.clone()).unwrap();
            for _ in 0..6 {
                tx.push(&[0; 9600]).unwrap();
            }
            tx.finish().unwrap();
            let (publisher, _clock) = crate::v561_stream_clock::channel();
            let call = Instant::now();
            let r = handle.play(source, epoch, op.clone(), publisher).unwrap();
            assert_eq!(r.played_samples, 28800);
            assert_eq!(r.samples, 28800);
            rows.push(serde_json::json!({"epoch":epoch,"first_pcm_ms":r.first_pcm_ms,"first_played_ms":r.first_played_ms,
            "entry_first_played_ms":r.first_played_at.saturating_duration_since(call).as_millis(),
            "gap_ms":previous.map(|p|r.first_played_at.saturating_duration_since(p).as_millis()),"marks":r.startup_marks}));
            previous = Some(r.completed_at);
        }
        session.finish().unwrap();
        let v = serde_json::json!({"scope":"silent_fixture_actual_device_diagnostic_not_avatar_or_voice_acceptance","rows":rows,"model_api_calls":0});
        std::fs::write(
            dir.join("profile.json"),
            serde_json::to_vec_pretty(&v).unwrap(),
        )
        .unwrap();
        println!("{}", v);
    }
    #[test]
    #[ignore = "explicit local device initialization only; no PCM or model calls"]
    fn profile_audio_session_startup_without_playback() {
        let binary =
            std::path::PathBuf::from(std::env::var_os("NAI_AUDIO_BINARY").expect("audio binary"));
        for _ in 0..3 {
            let session =
                super::Session::start(&binary, super::Operation::model_task(45000).unwrap(), 4)
                    .unwrap();
            println!(
                "audio_session_startup {}",
                serde_json::to_string(&session.startup).unwrap()
            );
            assert!(session.pid > 0);
            drop(session); // cancellation closes only this test's authenticated child
        }
    }
    use super::*;
    #[test]
    fn global_clock_maps_only_the_matching_script_and_source_range() {
        let id = SegmentIdentity {
            script_epoch: 7,
            sentence_id: 2,
            generation_epoch: 9,
            source_start_sample: 100,
        };
        let clock = PlaybackClockObservation {
            epoch: 7,
            device_generation: 1,
            observation_sequence: Some(10),
            state: ClockState::Running as i32,
            played_sample: Some(105),
            sample_rate: 24000,
        };
        let local = sentence_clock(clock, id, 10).unwrap();
        assert_eq!(local.epoch, 9);
        assert_eq!(local.played_sample, Some(5));
        for (epoch, played) in [(8, 105), (7, 99), (7, 111)] {
            let mut bad = clock;
            bad.epoch = epoch;
            bad.played_sample = Some(played);
            assert!(sentence_clock(bad, id, 10).is_err());
        }
    }
    #[test]
    fn waiting_actor_response_observes_cancel_and_disconnect_is_not_completion() {
        let op = Operation::model_task(1000).unwrap();
        let cancel = op.clone();
        let (_tx, rx) = mpsc::sync_channel::<Result<(), Error>>(1);
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            cancel.cancel();
        });
        let began = Instant::now();
        assert!(matches!(receive(&rx, &op), Err(Error::Cancelled)));
        assert!(began.elapsed() < Duration::from_millis(500));
        t.join().unwrap();
        let (tx, rx) = mpsc::sync_channel::<Result<(), Error>>(1);
        drop(tx);
        assert!(matches!(
            receive(&rx, &Operation::model_task(1000).unwrap()),
            Err(Error::OutcomeUnknown)
        ));
    }
}
