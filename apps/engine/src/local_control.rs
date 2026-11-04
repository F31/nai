//! Private stdin/stdout adapter. Blocking pipe I/O never owns the shutdown path.
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use nai_contracts::v1::{LocalControlFrame as Frame, LocalControlKind as Kind};
use nai_control_api::ControlApi;
use nai_control_ipc::{
    CALL_TIMEOUT, PEER_TIMEOUT, QUEUE_LIMIT, START_TIMEOUT, frame, read_frame, validate,
    write_frame,
};
use std::{
    sync::{Arc, mpsc},
    thread,
    time::Instant,
};
use tokio::sync::{mpsc as queue, oneshot};

type Received = Result<(Frame, Instant), String>;
type Outgoing = (Frame, oneshot::Sender<Result<(), String>>);

pub struct InheritedControl {
    pub token: String,
    input: queue::Receiver<Received>,
    output: mpsc::SyncSender<Outgoing>,
}

impl InheritedControl {
    pub async fn accept() -> Result<Self, String> {
        let (sender, mut input) = queue::channel(QUEUE_LIMIT);
        thread::spawn(move || {
            let mut stdin = std::io::stdin().lock();
            loop {
                let next = read_frame(&mut stdin)
                    .and_then(|value| value.ok_or_else(|| "IPC_EOF".into()))
                    .map(|value| (value, Instant::now()));
                let end = next.is_err();
                if sender.blocking_send(next).is_err() || end {
                    break;
                }
            }
        });
        let (bootstrap, _) = tokio::time::timeout(START_TIMEOUT, input.recv())
            .await
            .map_err(|_| "IPC_BOOTSTRAP_TIMEOUT")?
            .ok_or("IPC_EOF")??;
        if validate(&bootstrap)? != Kind::LocalControlBootstrap {
            return Err("IPC_BOOTSTRAP_REQUIRED".into());
        }
        let (output, outgoing) = mpsc::sync_channel::<Outgoing>(1);
        thread::spawn(move || {
            let mut stdout = std::io::stdout().lock();
            while let Ok((value, ack)) = outgoing.recv() {
                let result = write_frame(&mut stdout, &value);
                let broken = result.is_err();
                let _ = ack.send(result);
                if broken {
                    break;
                }
            }
        });
        Ok(Self {
            token: bootstrap.bootstrap_token,
            input,
            output,
        })
    }

    async fn send(&self, value: Frame) -> Result<(), String> {
        let (ack, response) = oneshot::channel();
        self.output
            .try_send((value, ack))
            .map_err(|_| "IPC_OUTPUT_BUSY")?;
        tokio::time::timeout(CALL_TIMEOUT, response)
            .await
            .map_err(|_| "IPC_OUTPUT_TIMEOUT")?
            .map_err(|_| "IPC_OUTPUT_CLOSED")?
    }

    pub async fn serve(mut self, api: Arc<ControlApi>) -> Result<(), String> {
        self.send(frame(Kind::LocalControlReady)).await?;
        let mut sequence = 0u64;
        loop {
            let (request, received) = tokio::time::timeout(PEER_TIMEOUT, self.input.recv())
                .await
                .map_err(|_| "IPC_PEER_LEASE_EXPIRED")?
                .ok_or("IPC_EOF")??;
            let kind = validate(&request)?;
            sequence = sequence.checked_add(1).ok_or("IPC_SEQUENCE_EXHAUSTED")?;
            if request.seq != sequence {
                return Err("IPC_SEQUENCE_INVALID".into());
            }
            match kind {
                Kind::LocalControlHeartbeat | Kind::LocalControlShutdown => {
                    let mut response = frame(kind);
                    response.seq = sequence;
                    self.send(response).await?;
                    if kind == Kind::LocalControlShutdown {
                        return Ok(());
                    }
                }
                Kind::LocalControlRequest => {
                    let deadline =
                        received + std::time::Duration::from_millis(request.ttl_ms.into());
                    if Instant::now() >= deadline {
                        return Err("IPC_REQUEST_EXPIRED".into());
                    }
                    let http = hyper::Request::builder()
                        .method(request.method.as_str())
                        .uri(&request.path)
                        .header("authorization", format!("Bearer {}", self.token))
                        .header("content-type", "application/json")
                        .body(Full::new(Bytes::from(request.body)))
                        .map_err(|_| "IPC_REQUEST_INVALID")?;
                    let response = tokio::time::timeout_at(deadline.into(), api.handle(http))
                        .await
                        .map_err(|_| "IPC_DISPATCH_TIMEOUT")?
                        .unwrap_or_else(|never| match never {});
                    let status = response.status().as_u16();
                    let body = response
                        .into_body()
                        .collect()
                        .await
                        .map_err(|_| "IPC_RESPONSE_BODY")?
                        .to_bytes()
                        .to_vec();
                    let mut response = frame(Kind::LocalControlResponse);
                    response.seq = sequence;
                    response.request_id = request.request_id;
                    response.status = status.into();
                    response.body = body;
                    self.send(response).await?;
                }
                _ => return Err("IPC_UNEXPECTED_KIND".into()),
            }
        }
    }
}
