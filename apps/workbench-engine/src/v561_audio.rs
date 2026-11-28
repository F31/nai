//! Spawn-bound raw PCM transfer and short-lease audio execution.
use nai_contracts::v561_media::PlaybackClockObservation;
use nai_contracts::workbench::WorkbenchError as Error;
use nai_runtime_core::foundation::Operation;
use std::path::Path;

pub fn play(binary: &Path, samples: &[i16], operation: Operation) -> Result<u64, Error> {
    play_observed(binary, samples, operation, |_| Ok(()))
}

/// Trusted composition supplies the same nonzero epoch to frame and audio owners.
pub fn play_in_epoch(
    binary: &Path,
    samples: &[i16],
    operation: Operation,
    epoch: u64,
    observe: impl FnMut(PlaybackClockObservation) -> Result<(), Error>,
) -> Result<u64, Error> {
    if epoch == 0 {
        return Err(Error::InvalidArgument);
    }
    play_scoped(binary, samples, operation, Some(epoch), observe)
}

pub fn play_observed(
    binary: &Path,
    samples: &[i16],
    operation: Operation,
    observe: impl FnMut(PlaybackClockObservation) -> Result<(), Error>,
) -> Result<u64, Error> {
    play_scoped(binary, samples, operation, None, observe)
}

#[cfg(not(windows))]
fn play_scoped(
    _binary: &Path,
    _samples: &[i16],
    _operation: Operation,
    _epoch: Option<u64>,
    _observe: impl FnMut(PlaybackClockObservation) -> Result<(), Error>,
) -> Result<u64, Error> {
    Err(Error::CapabilityUnavailable)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use nai_contracts::{
        v561_media::{ClockState, FrameRef, PixelFormat},
        v561_media_validation::FrameLimits,
    };
    use nai_media_core::v561_frame_scheduler::Scheduler;
    use sha2::{Digest, Sha256};
    #[test]
    #[ignore = "explicit Windows device test: synthetic frames and silent PCM"]
    fn independent_audio_clock_drives_frame_scheduler_in_current_task() {
        let binary = std::path::PathBuf::from(
            std::env::var_os("NAI_AUDIO_OUTPUT").expect("set NAI_AUDIO_OUTPUT"),
        );
        let mut tasks = crate::v561_tasks::Tasks::default();
        let task = tasks
            .begin(nai_kernel::output_owner::Speaker::Narration, 4000)
            .unwrap();
        let mut scheduler: Option<Scheduler> = None;
        let mut selected = std::collections::BTreeSet::new();
        let mut last_state = None;
        let count = play_observed(&binary, &vec![0; 24000], task.operation(), |clock| {
            last_state = Some(clock.state);
            if clock.state == ClockState::Unavailable as i32 {
                let mut frames = Scheduler::new(
                    "clock-fixture".into(),
                    clock.epoch,
                    clock.device_generation,
                    FrameLimits {
                        slots: 4,
                        max_width: 2,
                        max_height: 2,
                        max_payload_bytes: 16,
                        max_sample_rate: 24000,
                    },
                    4,
                    6000,
                )
                .unwrap();
                assert!(frames.present(&clock).unwrap().is_none());
                for seq in 0..4u64 {
                    let pixels = vec![seq as u8; 16];
                    frames
                        .offer(
                            FrameRef {
                                pool_id: "clock-fixture".into(),
                                epoch: clock.epoch,
                                sequence: Some(seq),
                                source_sample: Some(seq * 6000),
                                source_sample_rate: 24000,
                                pts_ns: Some(seq * 250_000_000),
                                width: 2,
                                height: 2,
                                stride_bytes: 8,
                                pixel_format: PixelFormat::Bgra8 as i32,
                                slot_index: Some(seq as u32),
                                slot_generation: 1,
                                payload_bytes: 16,
                                content_sha256: Sha256::digest(&pixels).to_vec(),
                            },
                            pixels,
                        )
                        .unwrap();
                }
                scheduler = Some(frames);
            } else {
                let frame = scheduler.as_mut().unwrap().present(&clock).unwrap();
                if let Some(frame) = frame {
                    selected.insert(frame.metadata.sequence.unwrap());
                }
                if clock.state == ClockState::Stopped as i32 {
                    assert!(frame.is_none());
                }
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(count, 24000);
        assert_eq!(selected, std::collections::BTreeSet::from([0, 1, 2, 3]));
        assert_eq!(last_state, Some(ClockState::Stopped as i32));
        tasks.finish(&task).unwrap();
    }
}

#[cfg(windows)]
fn play_scoped(
    binary: &Path,
    samples: &[i16],
    operation: Operation,
    bound_epoch: Option<u64>,
    mut observe: impl FnMut(PlaybackClockObservation) -> Result<(), Error>,
) -> Result<u64, Error> {
    use nai_contracts::v561_audio::{
        Command as Packet, Prepare, Receipt, Renew, State, command::Action,
    };
    use prost::Message;
    use ring::hmac;
    use ring::rand::{SecureRandom, SystemRandom};
    use sha2::{Digest, Sha256};
    use std::{
        io::Write,
        os::windows::process::CommandExt,
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::windows::named_pipe::ClientOptions,
    };
    use zeroize::Zeroizing;
    struct Reap {
        child: Child,
        writer: Option<std::thread::JoinHandle<()>>,
    }
    impl Drop for Reap {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            if let Some(writer) = self.writer.take() {
                let _ = writer.join();
            }
        }
    }
    operation.check()?;
    if !binary.is_absolute()
        || binary.file_name().and_then(|n| n.to_str()) != Some("nai-v561-audio-output.exe")
        || !binary.is_file()
        || samples.is_empty()
        || samples.len() > 480000
    {
        return Err(Error::InvalidArgument);
    }
    let count = samples.len() as u64;
    let bytes = Zeroizing::new(
        samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>(),
    );
    let hash = Sha256::digest(bytes.as_slice()).to_vec();
    let mut random = Zeroizing::new([0u8; 64]);
    SystemRandom::new()
        .fill(&mut *random)
        .map_err(|_| Error::IoFailed)?;
    let hex = |data: &[u8]| data.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let name = format!(r"\\.\pipe\nai-v561-audio-{}", hex(&random[..32]));
    let epoch = bound_epoch
        .unwrap_or(u64::from_le_bytes(random[..8].try_into().map_err(|_| Error::IoFailed)?) | 1);
    let token = Zeroizing::new(random[32..].to_vec());
    let key = hmac::Key::new(hmac::HMAC_SHA256, &token);
    let mut command = Command::new(binary);
    command.env_clear();
    for key in ["SystemRoot", "WINDIR"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .arg("--pipe")
        .arg(&name)
        .env("NAI_V561_AUDIO_TOKEN", hex(&token))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x08000000);
    let mut child = Reap {
        child: command.spawn().map_err(|_| Error::CapabilityUnavailable)?,
        writer: None,
    };
    let mut input = child.child.stdin.take().ok_or(Error::IoFailed)?;
    let check = operation.clone();
    child.writer = Some(std::thread::spawn(move || {
        for chunk in bytes.chunks(8192) {
            if check.check().is_err() || input.write_all(chunk).is_err() {
                break;
            }
        }
    }));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Error::CapabilityUnavailable)?;
    runtime.block_on(async {
        let start = Instant::now();
        let mut pipe = loop {
            operation.check()?;
            match ClientOptions::new().open(&name) {
                Ok(pipe) => break pipe,
                Err(_) => {
                    if start.elapsed() > Duration::from_secs(3)
                        || child
                            .child
                            .try_wait()
                            .map_err(|_| Error::IoFailed)?
                            .is_some()
                    {
                        return Err(Error::CapabilityUnavailable);
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        };
        let mut sequence = 1;
        let mut clock_bridge = nai_media_core::v561_clock_bridge::ClockBridge::new(
            epoch,
            count,
            24000,
            Duration::from_millis(200),
        )
        .ok_or(Error::InvalidArgument)?;
        let mut action = Action::Prepare(Prepare {
            samples: count,
            sha256: hash.clone(),
        });
        loop {
            operation.check()?;
            let mut packet = Packet {
                version: 1,
                mac: Vec::new(),
                epoch,
                sequence,
                action: Some(action),
            };
            packet.mac = hmac::sign(&key, &packet.encode_to_vec()).as_ref().to_vec();
            let bytes = Zeroizing::new(packet.encode_to_vec());
            let exchange_started = Instant::now();
            let exchange = async {
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
                Receipt::decode(bytes.as_slice()).map_err(|_| Error::ProtocolError)
            };
            let cancellation = async {
                loop {
                    if let Err(error) = operation.check() {
                        break error;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            };
            let mut receipt = tokio::select! {
                result=exchange=>result?,
                error=cancellation=>return Err(error),
                _=tokio::time::sleep(Duration::from_secs(3))=>return Err(Error::DeadlineExceeded),
            };
            operation.check()?;
            if receipt.state != State::Prepared as i32
                && exchange_started.elapsed() > Duration::from_millis(200)
            {
                return Err(Error::DeadlineExceeded);
            }
            let mac = std::mem::take(&mut receipt.mac);
            hmac::verify(&key, &receipt.encode_to_vec(), &mac)
                .map_err(|_| Error::AuthenticationFailed)?;
            let clock = receipt.clock.ok_or(Error::ProtocolError)?;
            if clock.played_sample.unwrap_or(0) != receipt.played_samples {
                return Err(Error::ProtocolError);
            }
            clock_bridge
                .accept(clock)
                .map_err(|_| Error::ProtocolError)?;
            if receipt.version != 1
                || receipt.epoch != epoch
                || receipt.sequence != sequence
                || receipt.samples != count
                || receipt.sha256 != hash
                || receipt.played_samples > count
            {
                return Err(Error::ProtocolError);
            }
            operation.check()?;
            use nai_contracts::v561_media::ClockState;
            if !matches!(
                (
                    State::try_from(receipt.state),
                    ClockState::try_from(clock.state)
                ),
                (Ok(State::Prepared), Ok(ClockState::Unavailable))
                    | (Ok(State::Running), Ok(ClockState::Running))
                    | (Ok(State::Completed), Ok(ClockState::Stopped))
            ) || (receipt.state == State::Completed as i32 && receipt.played_samples != count)
            {
                return Err(Error::ProtocolError);
            }
            observe(clock)?;
            match State::try_from(receipt.state) {
                Ok(State::Prepared) if sequence == 1 && receipt.played_samples == 0 => (),
                Ok(State::Running) if sequence > 1 => (),
                Ok(State::Completed) if sequence > 1 && receipt.played_samples == count => {
                    let mut ack = Packet {
                        version: 1,
                        mac: Vec::new(),
                        epoch,
                        sequence: sequence.checked_add(1).ok_or(Error::ResourceExhausted)?,
                        action: Some(Action::Stop(nai_contracts::v561_audio::Stop {})),
                    };
                    ack.mac = hmac::sign(&key, &ack.encode_to_vec()).as_ref().to_vec();
                    let bytes = Zeroizing::new(ack.encode_to_vec());
                    tokio::time::timeout(Duration::from_millis(100), async {
                        pipe.write_u32_le(bytes.len() as u32).await?;
                        pipe.write_all(&bytes).await?;
                        pipe.flush().await
                    })
                    .await
                    .map_err(|_| Error::DeadlineExceeded)?
                    .map_err(|_| Error::IoFailed)?;
                    return Ok(count);
                }
                _ => return Err(Error::ProtocolError),
            }
            sequence = sequence.checked_add(1).ok_or(Error::ResourceExhausted)?;
            action = Action::Renew(Renew { ttl_ms: 500 });
            if sequence > 2 {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    })
}
