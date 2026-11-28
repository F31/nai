//! One explicit audition per private child. Stop/EOF revokes before normal queues.
use crate::{
    v561_stream_audio::{Options, Report, audition_plan_in_task},
    v561_tasks::Tasks,
};
use nai_contracts::{
    v561_audition::{Command, Event, Kind, Request},
    workbench::WorkbenchError,
};
use nai_llm_openai::cosyvoice::Config;
use prost::Message;
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

enum Input {
    Start(Request),
    Stop(u64),
    Fault,
    Done(Result<Report, WorkbenchError>),
    Interject(u64, bool),
}
fn frame(input: &mut impl Read) -> Result<Option<Request>, ()> {
    let mut prefix = [0; 4];
    if input.read(&mut prefix[..1]).map_err(|_| ())? == 0 {
        return Ok(None);
    }
    input.read_exact(&mut prefix[1..]).map_err(|_| ())?;
    let n = u32::from_le_bytes(prefix) as usize;
    if n == 0 || n > 16384 {
        return Err(());
    }
    let mut bytes = vec![0; n];
    input.read_exact(&mut bytes).map_err(|_| ())?;
    Request::decode(bytes.as_slice()).map(Some).map_err(|_| ())
}
fn emit(
    out: &mpsc::SyncSender<Event>,
    sequence: u64,
    kind: Kind,
    code: &str,
    busy: bool,
    report: Option<&Report>,
) -> Result<(), ()> {
    out.try_send(Event {
        version: 1,
        request_sequence: sequence,
        task_id: 1,
        kind: kind as i32,
        code: code.into(),
        busy,
        samples: report.map(|r| r.samples),
        played_samples: report.map(|r| r.played_samples),
        config_selected: false,
        config_label: String::new(),
    })
    .map_err(|_| ())
}
pub fn run(config: PathBuf, audio: PathBuf) -> Result<(), &'static str> {
    run_inner(config, audio).map_err(|_| "NAI_AUDITION_CONTROL_FAILED")
}
fn run_inner(config: PathBuf, audio: PathBuf) -> Result<(), ()> {
    let mut raw = vec![];
    std::fs::File::open(config)
        .map_err(|_| ())?
        .take(32769)
        .read_to_end(&mut raw)
        .map_err(|_| ())?;
    if raw.len() > 32768 {
        return Err(());
    }
    let config: Config = serde_json::from_slice(&raw).map_err(|_| ())?;
    let mut tasks = Tasks::default();
    let stop = tasks.stop_handle();
    let input_stop = stop.clone();
    let (tx, rx) = mpsc::sync_channel(2);
    let input_tx = tx.clone();
    let shared_plan = std::sync::Arc::new(std::sync::Mutex::new(
        None::<nai_runtime_core::v561_narration::Plan>,
    ));
    let input_plan = shared_plan.clone();
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        let mut last = 0;
        loop {
            let value = match frame(&mut input) {
                Ok(Some(r)) if r.version == 1 && r.sequence == last + 1 && r.sequence <= 64 => r,
                _ => {
                    input_stop.stop();
                    let _ = input_tx.send(Input::Stop(last));
                    break;
                }
            };
            last = value.sequence;
            let message = match Command::try_from(value.command) {
                Ok(Command::Stop) if value.text.is_empty() => {
                    input_stop.stop();
                    Input::Stop(last)
                }
                Ok(Command::Start)
                    if !value.text.trim().is_empty() && value.text.chars().count() <= 1024 =>
                {
                    Input::Start(value)
                }
                Ok(Command::Interject)
                    if !value.text.trim().is_empty() && value.text.chars().count() <= 1024 =>
                {
                    let accepted = input_plan
                        .lock()
                        .ok()
                        .and_then(|p| p.as_ref().map(|p| p.interject(&value.text).is_ok()))
                        .unwrap_or(false);
                    Input::Interject(last, accepted)
                }
                _ => {
                    input_stop.stop();
                    Input::Fault
                }
            };
            let end = matches!(message, Input::Stop(_) | Input::Fault);
            if input_tx.send(message).is_err() || end {
                break;
            }
        }
    });
    let (out, events) = mpsc::sync_channel::<Event>(8);
    let (written, flushed) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut stdout = std::io::stdout().lock();
        for event in events {
            let data = event.encode_to_vec();
            if stdout
                .write_all(&(data.len() as u32).to_le_bytes())
                .and_then(|_| stdout.write_all(&data))
                .and_then(|_| stdout.flush())
                .is_err()
            {
                break;
            }
        }
        let _ = written.send(());
    });
    let mut running = None;
    let mut sequence = 0;
    let mut stopping = None;
    let mut used = false;
    let result = (|| {
        loop {
            if stopping.is_some_and(|deadline: Instant| Instant::now() >= deadline) {
                emit(&out, sequence, Kind::Failed, "STOP_TIMEOUT", true, None)?;
                return Err(());
            }
            let input = match rx.recv_timeout(Duration::from_millis(10)) {
                Ok(v) => v,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => return Err(()),
            };
            match input {
                Input::Start(request) => {
                    if used {
                        stop.stop();
                        return Err(());
                    }
                    used = true;
                    sequence = request.sequence;
                    let task = match tasks.begin(
                        nai_kernel::output_owner::Speaker::Narration,
                        config.deadline_ms,
                    ) {
                        Ok(t) => t,
                        Err(_) => {
                            emit(&out, sequence, Kind::Cancelled, "CANCELLED", false, None)?;
                            break;
                        }
                    };
                    let operation = task.operation();
                    let plan = match nai_runtime_core::v561_narration::Plan::new(
                        &request.text,
                        config.lookahead.clone(),
                        operation.clone(),
                    ) {
                        Ok(plan) => plan,
                        Err(error) => {
                            let _ = tasks.finish(&task);
                            emit(
                                &out,
                                sequence,
                                Kind::Failed,
                                error.as_str_name(),
                                false,
                                None,
                            )?;
                            break;
                        }
                    };
                    *shared_plan.lock().map_err(|_| ())? = Some(plan.clone());
                    running = Some(task);
                    emit(&out, sequence, Kind::Running, "STREAMING", true, None)?;
                    let (config, audio, done) = (config.clone(), audio.clone(), tx.clone());
                    std::thread::spawn(move || {
                        let result = audition_plan_in_task(
                            config,
                            plan,
                            &audio,
                            Options::default(),
                            operation,
                        );
                        let _ = done.send(Input::Done(result));
                    });
                }
                Input::Stop(seq) => {
                    stop.stop();
                    sequence = seq;
                    if running.is_none() {
                        emit(&out, sequence, Kind::Cancelled, "CANCELLED", false, None)?;
                        break;
                    }
                    emit(
                        &out,
                        sequence,
                        Kind::CancelRequested,
                        "STOP_REQUESTED",
                        true,
                        None,
                    )?;
                    stopping = Some(Instant::now() + Duration::from_secs(1));
                }
                Input::Interject(seq, accepted) => {
                    sequence = seq;
                    if stopping.is_none() && running.is_some() {
                        emit(
                            &out,
                            sequence,
                            Kind::Running,
                            if accepted {
                                "INTERJECT_QUEUED"
                            } else {
                                "INTERJECT_REJECTED"
                            },
                            true,
                            None,
                        )?;
                    }
                }
                Input::Fault => {
                    stop.stop();
                    return Err(());
                }
                Input::Done(report) => {
                    let task = running.take().ok_or(())?;
                    let finish = tasks.finish(&task);
                    if stopping.is_some() || matches!(finish, Err(WorkbenchError::Cancelled)) {
                        emit(&out, sequence, Kind::Cancelled, "CANCELLED", false, None)?;
                    } else {
                        match report {
                            Ok(report) if finish.is_ok() => emit(
                                &out,
                                sequence,
                                Kind::Completed,
                                "COMPLETED",
                                false,
                                Some(&report),
                            )?,
                            Err(error) => emit(
                                &out,
                                sequence,
                                Kind::Failed,
                                error.as_str_name(),
                                false,
                                None,
                            )?,
                            _ => emit(
                                &out,
                                sequence,
                                Kind::Failed,
                                "TASK_NOT_CURRENT",
                                false,
                                None,
                            )?,
                        }
                    }
                    break;
                }
            }
        }
        Ok(())
    })();
    stop.stop();
    drop(out);
    let _ = flushed.recv_timeout(Duration::from_millis(250));
    result
}
