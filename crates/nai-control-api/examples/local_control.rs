//! Explicit engineering root, no mock providers or media devices.
use nai_control_api::{Access, ControlApi, serve};
use nai_kernel::session::Session;
use nai_media_core::MediaGate;
use std::{sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::watch};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("NAI_CONTROL_TOKEN")?;
    let access = Arc::new(Access::new(
        &token,
        "local-operator",
        &[
            "runtime.read",
            "session.read",
            "media.mute",
            "session.takeover",
        ],
        Duration::from_secs(3600),
    )?);
    let gate = MediaGate::new("local-session", 32);
    let api = Arc::new(ControlApi::new(
        Session::new("local-session", "local-workspace").map_err(|_| "cannot create session")?,
        gate.emergency(),
        access,
    )?);
    let port: u16 = std::env::var("NAI_CONTROL_PORT")
        .unwrap_or_else(|_| "4318".into())
        .parse()?;
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    println!(
        "nai engineering control: http://{}/api/v1; no media devices or plugins connected",
        listener.local_addr()?
    );
    let (_stop, shutdown) = watch::channel(false);
    serve(listener, api, shutdown).await?;
    Ok(())
}
