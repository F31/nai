//! Explicit single-sentence audio executor; raw PCM and control are separate.
#[cfg(windows)]
fn main() {
    if windows::run().is_err() {
        eprintln!("NAI_AUDIO_EXECUTION_FAILED");
        std::process::exit(1);
    }
}
#[cfg(not(windows))]
fn main() {
    std::process::exit(1);
}

#[cfg(windows)]
mod windows {
    use nai_audio_cpal::clocked_experiment::{ExperimentalFence, Probe, open_pcm_diagnostic};
    use nai_audio_cpal::v561_clock::DeviceClock;
    use nai_contracts::v561_audio::{Command, Receipt, State, command::Action};
    use nai_contracts::v561_media::{ClockState, PlaybackClockObservation};
    use nai_media_core::v561_audio_lease::AudioLease;
    use prost::Message;
    use ring::hmac;
    use sha2::{Digest, Sha256};
    use std::{
        io::Read,
        sync::{Arc, mpsc},
        time::Duration,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::windows::named_pipe::{NamedPipeServer, ServerOptions},
    };
    use zeroize::Zeroizing;

    async fn read(pipe: &mut NamedPipeServer) -> Result<Command, ()> {
        let n = pipe.read_u32_le().await.map_err(|_| ())? as usize;
        if n == 0 || n > 1024 {
            return Err(());
        }
        let mut bytes = Zeroizing::new(vec![0; n]);
        pipe.read_exact(&mut bytes).await.map_err(|_| ())?;
        Command::decode(bytes.as_slice()).map_err(|_| ())
    }
    async fn reply(
        pipe: &mut NamedPipeServer,
        epoch: u64,
        sequence: u64,
        state: State,
        source: (u64, &[u8]),
        clock: PlaybackClockObservation,
        key: &hmac::Key,
    ) -> Result<(), ()> {
        let mut receipt = Receipt {
            version: 1,
            epoch,
            sequence,
            state: state as i32,
            samples: source.0,
            sha256: source.1.to_vec(),
            played_samples: clock.played_sample.unwrap_or(0),
            mac: Vec::new(),
            clock: Some(clock),
        };
        receipt.mac = hmac::sign(key, &receipt.encode_to_vec()).as_ref().to_vec();
        let bytes = receipt.encode_to_vec();
        tokio::time::timeout(Duration::from_millis(100), async {
            pipe.write_u32_le(bytes.len() as u32).await?;
            pipe.write_all(&bytes).await?;
            pipe.flush().await
        })
        .await
        .map_err(|_| ())?
        .map_err(|_| ())
    }
    fn auth(command: &Command, key: &hmac::Key, epoch: u64, previous: u64) -> bool {
        let mut signed = command.clone();
        let mac = std::mem::take(&mut signed.mac);
        command.version == 1
            && hmac::verify(key, &signed.encode_to_vec(), &mac).is_ok()
            && command.epoch == epoch
            && epoch != 0
            && command.sequence > previous
    }
    pub fn run() -> Result<(), ()> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args.len() != 2 || args[0] != "--pipe" {
            return Err(());
        }
        let suffix = args[1]
            .strip_prefix(r"\\.\pipe\nai-v561-audio-")
            .ok_or(())?;
        if suffix.len() != 64 || !suffix.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(());
        }
        let secret = Zeroizing::new(std::env::var("NAI_V561_AUDIO_TOKEN").map_err(|_| ())?);
        if secret.len() != 64 || !secret.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(());
        }
        let token = Zeroizing::new(
            (0..64)
                .step_by(2)
                .map(|i| u8::from_str_radix(&secret[i..i + 2], 16).map_err(|_| ()))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| ())?;
        let key = hmac::Key::new(hmac::HMAC_SHA256, &token);
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
            let first = tokio::time::timeout(Duration::from_secs(3), read(&mut pipe))
                .await
                .map_err(|_| ())??;
            if !auth(&first, &key, first.epoch, 0) || first.sequence != 1 {
                return Err(());
            }
            let Some(Action::Prepare(prepare)) = first.action else {
                return Err(());
            };
            if prepare.samples == 0 || prepare.samples > 480000 || prepare.sha256.len() != 32 {
                return Err(());
            }
            let n = prepare.samples as usize;
            let (sent, received) = mpsc::sync_channel(1);
            std::thread::spawn(move || {
                let result = (|| {
                    let mut bytes = Zeroizing::new(vec![0; n * 2]);
                    let mut input = std::io::stdin().lock();
                    input.read_exact(&mut bytes).map_err(|_| ())?;
                    let mut extra = [0];
                    if input.read(&mut extra).map_err(|_| ())? != 0 {
                        return Err(());
                    }
                    Ok(bytes)
                })();
                let _ = sent.send(result);
            });
            let bytes = received
                .recv_timeout(Duration::from_secs(3))
                .map_err(|_| ())??;
            if Sha256::digest(bytes.as_slice()).as_slice() != prepare.sha256 {
                return Err(());
            }
            let mut pcm = Some(Zeroizing::new(
                bytes
                    .chunks_exact(2)
                    .map(|b| i16::from_le_bytes([b[0], b[1]]))
                    .collect::<Vec<_>>(),
            ));
            let lease = Arc::new(AudioLease::new(first.epoch).ok_or(())?);
            let fence = Arc::new(ExperimentalFence::externally_controlled());
            let mut probe: Option<Probe> = None;
            let execution = async {
                // Initialize the device while the fence is still unarmed. Device
                // startup must not consume the first short playback lease.
                let check = lease.clone();
                probe = Some(
                    open_pcm_diagnostic(
                        fence.clone(),
                        pcm.take().ok_or(())?,
                        Arc::new(move || check.allowed()),
                        480000,
                    )
                    .map_err(|_| ())?,
                );
                let active_probe = probe.as_ref().ok_or(())?;
                let mut clock =
                    DeviceClock::new(first.epoch, active_probe.clock_reading().stream_generation)
                        .ok_or(())?;
                reply(
                    &mut pipe,
                    first.epoch,
                    1,
                    State::Prepared,
                    (prepare.samples, &prepare.sha256),
                    clock.observe_prepared(active_probe).ok_or(())?,
                    &key,
                )
                .await?;
                let mut previous = 1;
                let mut activated = false;
                loop {
                    let command = tokio::time::timeout(Duration::from_millis(500), read(&mut pipe))
                        .await
                        .map_err(|_| ())??;
                    if !auth(&command, &key, first.epoch, previous) {
                        return Err(());
                    }
                    previous = command.sequence;
                    match command.action {
                        Some(Action::Stop(_)) => {
                            lease.revoke();
                            fence.revoke();
                            reply(
                                &mut pipe,
                                first.epoch,
                                previous,
                                State::Stopped,
                                (prepare.samples, &prepare.sha256),
                                clock.observe_stopped(probe.as_ref().ok_or(())?).ok_or(())?,
                                &key,
                            )
                            .await?;
                            // Permit the peer to consume the terminal receipt before close.
                            let _ =
                                tokio::time::timeout(Duration::from_millis(250), read(&mut pipe))
                                    .await;
                            return Ok(());
                        }
                        Some(Action::Renew(renew)) => {
                            if !lease.renew(first.epoch, previous, renew.ttl_ms) {
                                return Err(());
                            }
                            if !activated {
                                if !fence.activate() {
                                    return Err(());
                                }
                                activated = true;
                            }
                            let probe = probe.as_ref().ok_or(())?;
                            let observation =
                                clock.observe_sentence(probe, prepare.samples).ok_or(())?;
                            let complete = observation.state == ClockState::Stopped as i32;
                            reply(
                                &mut pipe,
                                first.epoch,
                                previous,
                                if complete {
                                    State::Completed
                                } else {
                                    State::Running
                                },
                                (prepare.samples, &prepare.sha256),
                                observation,
                                &key,
                            )
                            .await?;
                            if complete {
                                lease.revoke();
                                fence.revoke();
                                // Closing a Windows pipe can discard an unread final receipt.
                                // Keep it alive until the parent acknowledges that receipt.
                                let ack = tokio::time::timeout(
                                    Duration::from_millis(250),
                                    read(&mut pipe),
                                )
                                .await
                                .map_err(|_| ())??;
                                if !auth(&ack, &key, first.epoch, previous)
                                    || !matches!(ack.action, Some(Action::Stop(_)))
                                {
                                    return Err(());
                                }
                                return Ok(());
                            }
                        }
                        _ => return Err(()),
                    }
                }
            }
            .await;
            lease.revoke();
            fence.revoke();
            if let Some(mut probe) = probe {
                probe
                    .stop_within(Duration::from_millis(500))
                    .map_err(|_| ())?;
            }
            execution
        })
    }
}
