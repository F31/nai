#[path = "../audition.rs"]
mod audition;
// Authenticated sole writer; private bounded lanes have no database access.
#[path = "../llm_lane.rs"]
mod llm_lane;
#[path = "../tts_lane.rs"]
mod tts_lane;
use nai_contracts::{workbench::*, workbench_wire as wire};
use nai_foundation_sqlite::{Error, Store};
use nai_runtime_core::foundation::{Operation, StopFence, uuid};
use std::{
    io::{self, Read},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
fn problem(request_id: String, seq: u64, code: Error) -> FoundationResponse {
    FoundationResponse {
        request_id,
        seq,
        outcome: Some(foundation_response::Outcome::Problem(WorkbenchProblem {
            code: code as i32,
            safe_message: code.as_str_name().into(),
            retryable: false,
            request_id: String::new(),
            field_errors: vec![],
        })),
    }
}
fn emit(response: FoundationResponse) -> Result<(), Error> {
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = wire::write(&mut io::stdout().lock(), &response);
        let _ = tx.send(result);
    });
    rx.recv_timeout(Duration::from_secs(4))
        .map_err(|_| Error::DeadlineExceeded)?
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{}", e.as_str_name());
        std::process::exit(1);
    }
}
fn run() -> Result<(), Error> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(2..=4).contains(&args.len())
        || args[0] != "--data-root"
        || args[2..]
            .iter()
            .any(|x| x != "--initialize" && x != "--desktop-owner")
        || args[2..]
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != args.len() - 2
    {
        return Err(Error::InvalidArgument);
    }
    let root = PathBuf::from(&args[1]);
    let token = std::env::var("NAI_WORKBENCH_TOKEN").map_err(|_| Error::AuthenticationFailed)?;
    let principal =
        std::env::var("NAI_FOUNDATION_PRINCIPAL").map_err(|_| Error::AuthenticationFailed)?;
    uuid(&principal)?;
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::AuthenticationFailed);
    }
    let fence = Arc::new(StopFence::default());
    let initialize = args[2..].iter().any(|x| x == "--initialize");
    let desktop = args[2..].iter().any(|x| x == "--desktop-owner");
    let request_limit = if desktop { 1_000_000 } else { 64 };
    let mut store = Store::open(&root, &principal, initialize, fence.clone())?;
    let scope = format!(
        "{}:{}",
        store.root_owner(),
        root.canonicalize()
            .map_err(|_| Error::DataRootRejected)?
            .display()
    );
    store.attach_tts_vault(Arc::new(nai_llm_openai::vault::NativeVault::for_speech(
        scope.clone(),
    )));
    store.attach_llm_vault(Arc::new(nai_llm_openai::vault::NativeVault::new(
        scope.clone(),
    )));
    let allow_loopback =
        cfg!(debug_assertions) && std::env::var("NAI_LLM_TEST_LOOPBACK").as_deref() == Ok("1");
    let mut lane = llm_lane::Lane::new(allow_loopback);
    let mut speech = tts_lane::Lane::new(allow_loopback, &token, fence.clone(), &root);
    // Authenticated root-parent priority path. Merely stopping never waits for the DB writer.
    let done = Arc::new(AtomicBool::new(false));
    let child_done = done.clone();
    let stop_fence = fence.clone();
    let stop_file = root.join("stop.request");
    let watch = std::thread::spawn(move || {
        while !child_done.load(Ordering::Acquire) {
            if stop_file.exists() {
                stop_fence.stop();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut input = io::stdin().lock();
        for _ in 0..request_limit {
            let mut first = [0u8; 1];
            match input.read(&mut first) {
                Ok(0) => {
                    let _ = tx.send(None);
                    break;
                }
                Ok(_) => (),
                Err(_) => {
                    let _ = tx.send(Some((Err(Error::IoFailed), Instant::now())));
                    break;
                }
            }
            let received = Instant::now();
            let mut frame = io::Cursor::new(first).chain(&mut input);
            let r = wire::read::<FoundationRequest>(&mut frame);
            let end = r.is_err();
            if tx.send(Some((r, received))).is_err() || end {
                break;
            }
        }
    });
    let started = Instant::now();
    let mut expected = 1;
    let result = (|| {
        while expected <= request_limit && (desktop || started.elapsed() < Duration::from_secs(60))
        {
            lane.poll(&mut store, &principal)?;
            speech.poll(&mut store, &principal)?;
            let (mut request, received) = match rx.recv_timeout(if desktop {
                Duration::from_millis(100)
            } else {
                Duration::from_secs(4)
            }) {
                Ok(Some((Ok(r), time))) => (r, time),
                Ok(None) => break,
                Ok(Some((Err(e), _))) => {
                    emit(problem(String::new(), 0, e))?;
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) if desktop => continue,
                Err(_) => return Err(Error::DeadlineExceeded),
            };
            let fatal = if request.protocol_major != wire::VERSION {
                Some(Error::ProtocolIncompatible)
            } else if request.authorization.len() != 64
                || request
                    .authorization
                    .iter()
                    .zip(token.as_bytes())
                    .fold(0u8, |a, (x, y)| a | (x ^ y))
                    != 0
            {
                Some(Error::AuthenticationFailed)
            } else if request.seq != expected {
                Some(Error::ProtocolError)
            } else if uuid(&request.request_id).is_err() {
                Some(Error::InvalidArgument)
            } else {
                None
            };
            if let Some(e) = fatal {
                emit(problem(String::new(), 0, e))?;
                break;
            }
            expected += 1;
            let outcome = (|| {
                if request.ttl_ms == 0 || request.ttl_ms > 4000 {
                    return Err(Error::InvalidArgument);
                }
                let spent = u32::try_from(received.elapsed().as_millis())
                    .map_err(|_| Error::DeadlineExceeded)?;
                let remaining = request
                    .ttl_ms
                    .checked_sub(spent)
                    .filter(|n| *n > 0)
                    .ok_or(Error::DeadlineExceeded)?;
                let op = Operation::new(remaining)?;
                let command = request.command.as_ref().ok_or(Error::InvalidArgument)?;
                lane.before(&mut store, &principal, command)?;
                speech.before(&mut store, &principal, command)?;
                let mut result =
                    if let Some(foundation_command::Operation::PreparePublicOutputBinding(value)) =
                        command.operation.as_ref()
                    {
                        if !value {
                            return Err(Error::InvalidArgument);
                        }
                        speech.prepare_output_binding(store.public_media_tick())?
                    } else if let Some(foundation_command::Operation::ConfirmLiveAuto(x)) =
                        &command.operation
                    {
                        speech.confirm_live_auto(&mut store, &principal, &op, x)?
                    } else if let Some(foundation_command::Operation::StartPublicExecution(x)) =
                        &command.operation
                    {
                        speech.execute_public(&mut store, &principal, &op, x, true)?
                    } else if let Some(foundation_command::Operation::RenewPublicExecution(x)) =
                        &command.operation
                    {
                        speech.execute_public(&mut store, &principal, &op, x, false)?
                    } else if let Some(foundation_command::Operation::StopPublicExecution(x)) =
                        &command.operation
                    {
                        speech.stop_public(&mut store, &principal, &op, x)?
                    } else if let Some(foundation_command::Operation::PreparePublicWindow(x)) =
                        command.operation.as_ref()
                    {
                        speech.prepare_public_window(&mut store, &principal, &op, x)?
                    } else if let Some(foundation_command::Operation::ConfirmPublicApproval(x)) =
                        command.operation.as_ref()
                    {
                        speech.confirm_public_approval(&mut store, &principal, &op, x)?
                    } else if let Some(foundation_command::Operation::RevokePublicApproval(x)) =
                        command.operation.as_ref()
                    {
                        speech.revoke_public_approval(&mut store, &principal, &op, x)?
                    } else if let Some(foundation_command::Operation::CheckPublicCandidate(x)) =
                        command.operation.as_ref()
                    {
                        speech.public_candidate(&mut store, &principal, &op, x)?
                    } else if matches!(
                        command.operation,
                        Some(
                            foundation_command::Operation::AuthorizeTtsAudio(_)
                                | foundation_command::Operation::RevokeTtsAudio(_)
                                | foundation_command::Operation::StartTtsAudition(_)
                                | foundation_command::Operation::RenewTtsAudition(_)
                                | foundation_command::Operation::StopTtsAudition(_)
                                | foundation_command::Operation::SelectOutputDevice(_)
                        )
                    ) {
                        speech.audio(&mut store, &principal, &op, command)?
                    } else {
                        store.execute(&op, &principal, command)?
                    };
                lane.after(&mut store, &principal, command, &mut result)?;
                speech.after(&mut store, &principal, command, &mut result)?;
                if let Some(foundation_command::Operation::Dispatch(x)) = &command.operation
                    && result.state == DeliveryState::Committed as i32
                {
                    result = store.project_private_card(
                        &op,
                        &principal,
                        &x.workspace_id,
                        &x.delivery_id,
                    )?;
                }
                op.check()?;
                Ok(result)
            })();
            if let Some(FoundationCommand {
                operation: Some(foundation_command::Operation::SaveLlmModel(x)),
                ..
            }) = request.command.as_mut()
            {
                use zeroize::Zeroize;
                x.api_key.zeroize();
            }
            if let Some(FoundationCommand {
                operation: Some(foundation_command::Operation::SaveTtsModel(x)),
                ..
            }) = request.command.as_mut()
            {
                use zeroize::Zeroize;
                x.api_key.zeroize();
            }
            let response = match outcome {
                Ok(result) => FoundationResponse {
                    request_id: request.request_id,
                    seq: request.seq,
                    outcome: Some(foundation_response::Outcome::Result(result)),
                },
                Err(e) => problem(request.request_id, request.seq, e),
            };
            emit(response)?;
        }
        Ok(())
    })();
    lane.stop(&mut store, &principal);
    speech.stop(&mut store, &principal);
    done.store(true, Ordering::Release);
    watch.join().map_err(|_| Error::IoFailed)?;
    result
}
