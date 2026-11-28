//! Spawn-private incremental audio executor; no HTTP or provider credentials.
#[cfg(not(windows))]
fn main() {
    std::process::exit(1);
}
#[cfg(windows)]
fn main() {
    if windows::run().is_err() {
        eprintln!("NAI_STREAM_AUDIO_FAILED");
        std::process::exit(1);
    }
}

#[cfg(windows)]
mod windows {
    use nai_audio_cpal::stream_device::Device;
    use nai_contracts::{
        v561_media::{ClockState, PlaybackClockObservation},
        v561_stream_audio::{Command, Receipt, State, WireVersion, command::Action},
    };
    use nai_media_core::{stream_pcm::Buffer, v561_audio_lease::AudioLease};
    use prost::Message;
    use ring::hmac;
    use std::{
        io::Read,
        sync::{
            Arc, Mutex,
            atomic::{AtomicU64, Ordering},
            mpsc,
        },
        time::Duration,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::windows::named_pipe::ServerOptions,
    };

    pub fn run() -> Result<(), ()> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args.len() != 2 || args[0] != "--pipe" {
            return Err(());
        }
        let suffix = args[1]
            .strip_prefix(r"\\.\pipe\nai-v561-stream-")
            .ok_or(())?;
        if suffix.len() != 64 || !suffix.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(());
        }
        let secret =
            zeroize::Zeroizing::new(std::env::var("NAI_V561_AUDIO_TOKEN").map_err(|_| ())?);
        if secret.len() != 64 || !secret.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(());
        }
        let token = zeroize::Zeroizing::new(
            (0..64)
                .step_by(2)
                .map(|i| u8::from_str_radix(&secret[i..i + 2], 16).map_err(|_| ()))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let key = hmac::Key::new(hmac::HMAC_SHA256, &token);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| ())?;
        runtime.block_on(async {
            let mut pipe = ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .create(&args[1])
                .map_err(|_| ())?;
            tokio::time::timeout(Duration::from_secs(3), pipe.connect())
                .await
                .map_err(|_| ())?
                .map_err(|_| ())?;
            let first = read(&mut pipe, &key, 0, 0).await?;
            let (begin, mut segments) = match first.action {
                Some(Action::Begin(begin)) => (begin, None),
                Some(Action::OpenScript(open)) => (
                    nai_contracts::v561_stream_audio::Begin {
                        sample_rate: open.sample_rate,
                        capacity_samples: open.capacity_samples,
                        max_samples: open.max_samples,
                        prefill_samples: open.prefill_samples,
                    },
                    Some(
                        nai_media_core::continuous_audio::Timeline::new(
                            first.epoch,
                            open.max_samples,
                            open.max_sentences,
                        )
                        .map_err(|_| ())?,
                    ),
                ),
                _ => return Err(()),
            };
            let buffer = if segments.is_some() {
                Buffer::new_script(&begin)
            } else {
                Buffer::new(&begin)
            };
            let buffer = Arc::new(Mutex::new(buffer.map_err(|_| ())?));
            let lease = Arc::new(AudioLease::new(first.epoch).ok_or(())?);
            struct Revoke(Arc<AudioLease>);
            impl Drop for Revoke {
                fn drop(&mut self) {
                    self.0.revoke();
                }
            }
            let until = Arc::new(AtomicU64::new(0));
            let device = Device::open(
                buffer.clone(),
                lease.clone(),
                until.clone(),
                begin.prefill_samples,
            )
            .map_err(|_| ())?;
            let _revoke = Revoke(lease.clone());
            let (tx, rx) = mpsc::sync_channel(1);
            std::thread::spawn(move || {
                let mut input = std::io::stdin().lock();
                loop {
                    let mut size = [0u8; 4];
                    if input.read_exact(&mut size).is_err() {
                        break;
                    }
                    let n = u32::from_le_bytes(size) as usize;
                    if n == 0 || n > 9600 || !n.is_multiple_of(2) {
                        break;
                    }
                    let mut bytes = zeroize::Zeroizing::new(vec![0u8; n]);
                    if input.read_exact(&mut bytes).is_err() || tx.send(bytes).is_err() {
                        break;
                    }
                }
            });
            let mut previous = 1;
            let mut state = State::Prepared;
            let mut segment_identity = None;
            let mut segment_chunk = 0u64;
            loop {
                let (received, free, sealed) = {
                    let data = buffer.lock().map_err(|_| ())?;
                    (data.received(), data.free() as u32, data.sealed())
                };
                let observation = device.observation().ok_or(())?;
                let played = observation.played_samples;
                let clock_age_ms = u32::try_from(
                    observation
                        .observed_at
                        .elapsed()
                        .as_nanos()
                        .div_ceil(1_000_000),
                )
                .map_err(|_| ())?;
                if device.failed.load(Ordering::Acquire) {
                    return Err(());
                }
                if sealed && played == received {
                    state = State::Completed;
                    lease.revoke();
                }
                let segment_progress = match segments.as_mut() {
                    Some(timeline) => timeline
                        .observe(device.submitted.load(Ordering::Acquire), played)
                        .map_err(|_| ())?,
                    None => Vec::new(),
                };
                let mut reply = Receipt {
                    segments: segment_progress,
                    version: WireVersion::Current as u32,
                    epoch: first.epoch,
                    sequence: previous,
                    state: state as i32,
                    received_samples: received,
                    submitted_samples: device.submitted.load(Ordering::Acquire),
                    played_samples: played,
                    free_samples: free,
                    sealed,
                    mac: vec![],
                    clock_age_ms,
                    clock: Some(PlaybackClockObservation {
                        epoch: first.epoch,
                        device_generation: device.generation,
                        observation_sequence: Some(previous),
                        state: match state {
                            State::Prepared => ClockState::Unavailable,
                            State::Running => ClockState::Running,
                            _ => ClockState::Stopped,
                        } as i32,
                        played_sample: if state == State::Prepared {
                            None
                        } else {
                            Some(played)
                        },
                        sample_rate: if state == State::Prepared { 0 } else { 24000 },
                    }),
                };
                reply.mac = hmac::sign(&key, &reply.encode_to_vec()).as_ref().to_vec();
                let bytes = reply.encode_to_vec();
                tokio::time::timeout(Duration::from_millis(100), async {
                    pipe.write_u32_le(bytes.len() as u32).await?;
                    pipe.write_all(&bytes).await?;
                    pipe.flush().await
                })
                .await
                .map_err(|_| ())?
                .map_err(|_| ())?;
                if let Some(timeline) = segments.as_mut() {
                    timeline.retire_completed().map_err(|_| ())?;
                }
                if matches!(state, State::Completed | State::Stopped) {
                    let _ = tokio::time::timeout(Duration::from_millis(250), pipe.read_u8()).await;
                    return Ok(());
                }
                let command = read(&mut pipe, &key, first.epoch, previous).await?;
                previous = command.sequence;
                match command.action {
                    Some(Action::Renew(renew)) => {
                        if renew.play_until_sample < until.load(Ordering::Acquire)
                            || renew.play_until_sample > received
                            || !lease.renew(first.epoch, previous, renew.ttl_ms)
                        {
                            return Err(());
                        }
                        until.store(renew.play_until_sample, Ordering::Release);
                        state = State::Running;
                    }
                    Some(Action::Chunk(meta)) => {
                        let bytes = rx
                            .recv_timeout(Duration::from_millis(100))
                            .map_err(|_| ())?;
                        let mut data = buffer.lock().map_err(|_| ())?;
                        data.push(&meta, &bytes).map_err(|_| ())?;
                        if let Some(timeline) = segments.as_mut() {
                            segment_chunk = segment_chunk.checked_add(1).ok_or(())?;
                            timeline
                                .push(segment_identity.as_ref().ok_or(())?, segment_chunk, &bytes)
                                .map_err(|_| ())?;
                        }
                    }
                    Some(Action::Seal(end)) => {
                        if segments.is_some() && segment_identity.is_some() {
                            return Err(());
                        }
                        buffer.lock().map_err(|_| ())?.seal(&end).map_err(|_| ())?
                    }
                    Some(Action::SegmentBegin(begin)) => {
                        segments.as_mut().ok_or(())?.begin(&begin).map_err(|_| ())?;
                        segment_identity = begin.identity;
                        segment_chunk = 0;
                    }
                    Some(Action::SegmentSeal(seal)) => {
                        if seal.identity != segment_identity {
                            return Err(());
                        }
                        segments.as_mut().ok_or(())?.seal(&seal).map_err(|_| ())?;
                        let mut data = buffer.lock().map_err(|_| ())?;
                        let endpoint = data.received();
                        data.mark_verified_boundary(endpoint).map_err(|_| ())?;
                        segment_identity = None;
                    }
                    Some(Action::Stop(_)) => {
                        lease.revoke();
                        state = State::Stopped;
                    }
                    _ => return Err(()),
                }
            }
        })
    }
    async fn read(
        pipe: &mut tokio::net::windows::named_pipe::NamedPipeServer,
        key: &hmac::Key,
        epoch: u64,
        previous: u64,
    ) -> Result<Command, ()> {
        tokio::time::timeout(Duration::from_millis(500), async {
            let n = pipe.read_u32_le().await.map_err(|_| ())? as usize;
            if n == 0 || n > 1024 {
                return Err(());
            }
            let mut bytes = vec![0; n];
            pipe.read_exact(&mut bytes).await.map_err(|_| ())?;
            let mut command = Command::decode(bytes.as_slice()).map_err(|_| ())?;
            let mac = std::mem::take(&mut command.mac);
            if command.version != WireVersion::Current as u32
                || command.epoch == 0
                || (epoch != 0 && command.epoch != epoch)
                || command.sequence != previous + 1
                || hmac::verify(key, &command.encode_to_vec(), &mac).is_err()
            {
                return Err(());
            }
            Ok(command)
        })
        .await
        .map_err(|_| ())?
    }
}
