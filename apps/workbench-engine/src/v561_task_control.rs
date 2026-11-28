//! Bounded private stdio control. Audio requires an explicit independent executor.
use crate::v561_tasks::{Task, Tasks};
use nai_contracts::v561_task::{Command, Event, EventKind, Request};
use nai_kernel::output_owner::Speaker;
use nai_llm_openai::{
    qwen::{self, Config},
    tts::Outcome,
};
use prost::Message;
use std::{
    io::{Read, Write},
    sync::mpsc,
    time::Duration,
};

const MAX_FRAME: usize = 16 * 1024;
enum Input {
    Request(Result<Option<Request>, ()>),
    Done(
        Outcome,
        Option<Result<u64, nai_contracts::workbench::WorkbenchError>>,
    ),
    Clock(u64, nai_contracts::v561_media::PlaybackClockObservation),
}
struct Running {
    task: Task,
    sequence: u64,
}

fn read_request(input: &mut impl Read) -> Result<Option<Request>, ()> {
    let mut prefix = [0; 4];
    // Clean EOF differs from a truncated prefix.
    match input.read(&mut prefix[..1]).map_err(|_| ())? {
        0 => return Ok(None),
        1 => (),
        _ => return Err(()),
    }
    input.read_exact(&mut prefix[1..]).map_err(|_| ())?;
    let len = u32::from_le_bytes(prefix) as usize;
    if len == 0 || len > MAX_FRAME {
        return Err(());
    }
    let mut bytes = vec![0; len];
    input.read_exact(&mut bytes).map_err(|_| ())?;
    Request::decode(bytes.as_slice()).map(Some).map_err(|_| ())
}

fn emit(
    tx: &mpsc::SyncSender<Event>,
    seq: u64,
    id: u64,
    kind: EventKind,
    code: &str,
    busy: bool,
    audio: Option<(u64, u32, u64)>,
) -> Result<(), ()> {
    let (samples, sample_rate, played_samples) = audio.unwrap_or_default();
    tx.try_send(Event {
        version: 1,
        request_sequence: seq,
        task_id: id,
        kind: kind as i32,
        code: code.into(),
        samples,
        sample_rate,
        worker_busy: busy,
        played_samples,
        clock: None,
    })
    .map_err(|_| ())
}

pub fn run(args: impl Iterator<Item = String>) -> Result<(), &'static str> {
    run_inner(args).map_err(|()| "NAI_TASK_CONTROL_FAILED")
}

fn run_inner(args: impl Iterator<Item = String>) -> Result<(), ()> {
    let mut config = Config::default();
    let mut audio_output: Option<std::path::PathBuf> = None;
    let mut clock_events = false;
    let mut args = args;
    while let Some(arg) = args.next() {
        let value = args.next().ok_or(())?;
        match arg.as_str() {
            "--endpoint" => config.endpoint = value,
            "--model" => config.model = value,
            "--voice" => config.voice = value,
            "--language" => config.language = value,
            "--deadline-ms" => config.deadline_ms = value.parse().map_err(|_| ())?,
            "--audio-output" => audio_output = Some(value.into()),
            "--clock-events" => clock_events = value.parse().map_err(|_| ())?,
            _ => return Err(()),
        }
    }
    if clock_events && audio_output.is_none() {
        return Err(());
    }
    if let Some(path) = &audio_output
        && (!path.is_absolute()
            || !path.is_file()
            || path.file_name().and_then(|n| n.to_str()) != Some("nai-v561-audio-output.exe"))
    {
        return Err(());
    }
    nai_runtime_core::tts::validate_profile("qwen_local", &config.endpoint, &config.language)
        .map_err(|_| ())?;
    nai_runtime_core::foundation::Operation::model_task(config.deadline_ms).map_err(|_| ())?;
    let (tx, rx) = mpsc::sync_channel(2);
    let input_tx = tx.clone();
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        loop {
            let request = read_request(&mut input);
            let end = !matches!(request, Ok(Some(_)));
            if input_tx.send(Input::Request(request)).is_err() || end {
                break;
            }
        }
    });
    let (output, events) = mpsc::sync_channel::<Event>(8);
    let (written, flushed) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut out = std::io::stdout().lock();
        for event in events {
            let bytes = event.encode_to_vec();
            if out
                .write_all(&(bytes.len() as u32).to_le_bytes())
                .and_then(|_| out.write_all(&bytes))
                .and_then(|_| out.flush())
                .is_err()
            {
                break;
            }
        }
        let _ = written.send(());
    });
    let mut tasks = Tasks::default();
    let mut running: Option<Running> = None;
    let mut human: Option<(u64, Task)> = None;
    let mut last = 0;
    let result = (|| {
        loop {
            match rx.recv().map_err(|_| ())? {
                Input::Request(Ok(None)) => break,
                Input::Request(Err(())) => return Err(()),
                Input::Clock(sequence, clock) => {
                    if running.as_ref().is_some_and(|pending| {
                        pending.sequence == sequence && pending.task.operation().check().is_ok()
                    }) {
                        output
                            .try_send(Event {
                                version: 1,
                                request_sequence: sequence,
                                task_id: sequence,
                                kind: EventKind::Clock as i32,
                                code: "DEVICE_CLOCK".into(),
                                played_samples: clock.played_sample.unwrap_or(0),
                                sample_rate: clock.sample_rate,
                                worker_busy: true,
                                clock: Some(clock),
                                ..Default::default()
                            })
                            .map_err(|_| ())?;
                    }
                }
                Input::Done(outcome, playback) => {
                    let pending = running.take().ok_or(())?;
                    let valid = tasks.finish(&pending.task);
                    let failure = outcome.error.or_else(|| {
                        playback
                            .as_ref()
                            .and_then(|result| result.as_ref().err().copied())
                    });
                    let (kind, code, audio) = if valid.is_err()
                        || failure == Some(nai_contracts::workbench::WorkbenchError::Cancelled)
                    {
                        (EventKind::Cancelled, "CANCELLED", None)
                    } else if failure.is_some() {
                        (
                            EventKind::Failed,
                            if playback.is_some() {
                                "AUDIO_EXECUTION_FAILED"
                            } else {
                                "TTS_FAILED"
                            },
                            None,
                        )
                    } else {
                        (
                            EventKind::Completed,
                            if playback.is_some() {
                                "AUDIO_PLAYBACK_COMPLETED"
                            } else {
                                "SYNTHESIZED_NOT_PLAYED"
                            },
                            Some((
                                outcome.samples.len() as u64,
                                outcome.sample_rate,
                                playback.and_then(Result::ok).unwrap_or(0),
                            )),
                        )
                    };
                    emit(
                        &output,
                        pending.sequence,
                        pending.sequence,
                        kind,
                        code,
                        false,
                        audio,
                    )?;
                }
                Input::Request(Ok(Some(request))) => {
                    let command = Command::try_from(request.command).ok();
                    let busy = running.is_some();
                    if request.version != 1 || request.sequence == 0 || request.sequence <= last {
                        emit(
                            &output,
                            request.sequence,
                            0,
                            EventKind::Rejected,
                            "INVALID_SEQUENCE_OR_VERSION",
                            busy,
                            None,
                        )?;
                        continue;
                    }
                    last = request.sequence;
                    if (command != Some(Command::Finish) && request.task_id != 0)
                        || (!matches!(command, Some(Command::Narrate | Command::Answer))
                            && !request.text.is_empty())
                    {
                        emit(
                            &output,
                            last,
                            0,
                            EventKind::Rejected,
                            "INVALID_ARGUMENT",
                            busy,
                            None,
                        )?;
                        continue;
                    }
                    match command {
                        Some(Command::Stop) => {
                            tasks.stop();
                            emit(
                                &output,
                                last,
                                0,
                                EventKind::Stopped,
                                "LOGICALLY_STOPPED",
                                busy,
                                None,
                            )?;
                            break;
                        }
                        Some(Command::Status) => emit(
                            &output,
                            last,
                            human
                                .as_ref()
                                .map(|v| v.0)
                                .or_else(|| running.as_ref().map(|v| v.sequence))
                                .unwrap_or(0),
                            EventKind::Status,
                            if audio_output.is_some() {
                                "AUDIO_OUTPUT_ENABLED"
                            } else {
                                "NO_MEDIA_OUTPUT"
                            },
                            busy,
                            None,
                        )?,
                        Some(Command::Takeover) => {
                            let task = tasks
                                .begin(Speaker::Human, config.deadline_ms)
                                .map_err(|_| ())?;
                            human = Some((last, task));
                            emit(
                                &output,
                                last,
                                last,
                                EventKind::Accepted,
                                "HUMAN_OWNER_NO_MIC",
                                busy,
                                None,
                            )?;
                        }
                        Some(Command::Finish) => {
                            let valid = if human.as_ref().is_some_and(|v| v.0 == request.task_id) {
                                let (_, task) = human.take().ok_or(())?;
                                tasks.finish(&task).is_ok()
                            } else {
                                false
                            };
                            emit(
                                &output,
                                last,
                                request.task_id,
                                if valid {
                                    EventKind::Completed
                                } else {
                                    EventKind::Rejected
                                },
                                if valid {
                                    "OWNER_RELEASED"
                                } else {
                                    "STALE_TASK"
                                },
                                busy,
                                None,
                            )?;
                        }
                        Some(Command::Narrate | Command::Answer) => {
                            if busy || nai_runtime_core::tts::characters(&request.text).is_err() {
                                emit(
                                    &output,
                                    last,
                                    0,
                                    EventKind::Rejected,
                                    if busy { "BUSY" } else { "INVALID_TEXT" },
                                    busy,
                                    None,
                                )?;
                                continue;
                            }
                            let speaker = if command == Some(Command::Answer) {
                                Speaker::Answer
                            } else {
                                Speaker::Narration
                            };
                            let task = match tasks.begin(speaker, config.deadline_ms) {
                                Ok(task) => task,
                                Err(_) => {
                                    emit(
                                        &output,
                                        last,
                                        0,
                                        EventKind::Rejected,
                                        "OWNER_DENIED",
                                        false,
                                        None,
                                    )?;
                                    continue;
                                }
                            };
                            let operation = task.operation();
                            running = Some(Running {
                                task,
                                sequence: last,
                            });
                            let config = config.clone();
                            let tx = tx.clone();
                            let audio_output = audio_output.clone();
                            emit(
                                &output,
                                last,
                                last,
                                EventKind::Accepted,
                                "SYNTHESIS_STARTED",
                                true,
                                None,
                            )?;
                            std::thread::spawn(move || {
                                let outcome =
                                    qwen::synthesize(&config, &request.text, operation.clone());
                                let playback = if outcome.error.is_none() {
                                    audio_output.map(|binary| {
                                        crate::v561_audio::play_observed(
                                            &binary,
                                            &outcome.samples,
                                            operation,
                                            |clock| {
                                                if clock_events {
                                                    let running_clock = clock.state == nai_contracts::v561_media::ClockState::Running as i32;
                                                    match tx.try_send(Input::Clock(request.sequence,clock)) {
                                                        Ok(()) => (),
                                                        Err(mpsc::TrySendError::Full(_)) if running_clock => (),
                                                        Err(mpsc::TrySendError::Full(_)) => return Err(nai_contracts::workbench::WorkbenchError::ResourceExhausted),
                                                        Err(mpsc::TrySendError::Disconnected(_)) => return Err(nai_contracts::workbench::WorkbenchError::Cancelled),
                                                    }
                                                }
                                                Ok(())
                                            },
                                        )
                                    })
                                } else {
                                    None
                                };
                                if let Some(Err(error)) = &playback {
                                    eprintln!("AUDIO_EXECUTION_ERROR {}", error.as_str_name());
                                }
                                let _ = tx.send(Input::Done(outcome, playback));
                            });
                        }
                        _ => emit(
                            &output,
                            last,
                            0,
                            EventKind::Rejected,
                            "UNKNOWN_COMMAND",
                            busy,
                            None,
                        )?,
                    }
                }
            }
        }
        Ok(())
    })();
    tasks.stop();
    drop(output);
    // A parent that stops reading must not keep shutdown blocked in stdout.
    let _ = flushed.recv_timeout(Duration::from_millis(250));
    result
}
