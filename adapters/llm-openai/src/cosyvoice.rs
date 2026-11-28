//! Explicit local multipart -> bounded canonical PCM. No retry or playback grant.
use futures_util::StreamExt;
use nai_contracts::workbench::WorkbenchError as Error;
use nai_runtime_core::foundation::Operation;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub endpoint: String,
    pub reference_file: PathBuf,
    pub prompt_text: String,
    pub deadline_ms: u32,
    pub max_samples: u64,
    #[serde(default)]
    pub lookahead: nai_runtime_core::v561_narration::Policy,
}
pub struct Summary {
    pub samples: u64,
    pub first_pcm_ms: u128,
    pub complete_ms: u128,
    pub transport_eof_ms: u128,
}
/// Observe an existing cancelled request draining; never retries synthesis.
pub fn wait_idle(config: &Config, operation: Operation) -> Result<(), Error> {
    let mut url = crate::endpoint(&config.endpoint, true)?;
    if url.scheme() != "http" || !matches!(url.host_str(), Some("127.0.0.1" | "localhost")) {
        return Err(Error::InvalidArgument);
    }
    url.set_path("/health");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Error::IoFailed)?;
    runtime.block_on(async {
        let client=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_millis(1000)).build().map_err(|_|Error::IoFailed)?;
        let poll=async {
            loop {
                operation.check()?;
                let response=client.get(url.clone()).send().await.map_err(|_|Error::CapabilityUnavailable)?;
                if !response.status().is_success(){return Err(Error::CapabilityUnavailable);}
                let mut chunks=response.bytes_stream();let mut bytes=Vec::new();
                while let Some(chunk)=chunks.next().await {let chunk=chunk.map_err(|_|Error::OutcomeUnknown)?;if chunk.len()>4096-bytes.len(){return Err(Error::ResourceExhausted);}bytes.extend_from_slice(&chunk);}
                let status:serde_json::Value=serde_json::from_slice(&bytes).map_err(|_|Error::ProtocolError)?;
                if status.get("status").and_then(|v|v.as_str())!=Some("ok") {return Err(Error::CapabilityUnavailable);}
                match status.get("busy").and_then(|v|v.as_bool()) {Some(false)=>return Ok(()),Some(true)=>{},None=>return Err(Error::ProtocolError)}
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        };
        tokio::pin!(poll);
        loop {tokio::select! {result=&mut poll=>return result,_=tokio::time::sleep(Duration::from_millis(10))=>operation.check()?,}}
    })
}
pub fn stream(
    config: &Config,
    text: &str,
    operation: Operation,
    mut accept: impl FnMut(&[u8]) -> Result<(), Error>,
) -> Result<Summary, Error> {
    let url = crate::endpoint(&config.endpoint, true)?;
    if url.scheme() != "http"
        || config.endpoint.len() > 256
        || !matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
        || url.path() != "/inference_zero_shot"
        || config.deadline_ms == 0
        || config.deadline_ms > 45000
        || config.max_samples == 0
        || config.max_samples > 480000
        || text.trim().is_empty()
        || text.chars().count() > 1024
        || config.prompt_text.is_empty()
        || config.prompt_text.chars().count() > 2048
        || !config.prompt_text.contains("<|endofprompt|>")
        || !config.reference_file.is_absolute()
        || config.reference_file.as_os_str().len() > 4096
    {
        return Err(Error::InvalidArgument);
    }
    operation.check()?;
    let file = std::fs::File::open(&config.reference_file).map_err(|_| Error::IoFailed)?;
    if file.metadata().map_err(|_| Error::IoFailed)?.len() > 10485760 {
        return Err(Error::ResourceExhausted);
    }
    use std::io::Read;
    let mut reference = Vec::new();
    file.take(10485761)
        .read_to_end(&mut reference)
        .map_err(|_| Error::IoFailed)?;
    if reference.is_empty() || reference.len() > 10485760 {
        return Err(Error::ResourceExhausted);
    }
    let boundary = "nai-cosyvoice-local-boundary";
    // Reject collisions rather than producing ambiguous multipart framing.
    if text.contains(boundary)
        || config.prompt_text.contains(boundary)
        || reference
            .windows(boundary.len())
            .any(|w| w == boundary.as_bytes())
    {
        return Err(Error::InvalidArgument);
    }
    let mut body = zeroize::Zeroizing::new(Vec::new());
    for (name, value) in [
        ("tts_text", text),
        ("prompt_text", config.prompt_text.as_str()),
        ("stream", "true"),
    ] {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"prompt_wav\"; filename=\"reference.wav\"\r\nContent-Type: audio/wav\r\n\r\n").as_bytes());
    body.extend_from_slice(&reference);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let started = Instant::now();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Error::IoFailed)?;
    runtime.block_on(async {
        let client=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_millis(config.deadline_ms.into())).build().map_err(|_|Error::IoFailed)?;
        let future=async {
            let response=client.post(url.clone()).header("Content-Type",format!("multipart/form-data; boundary={boundary}")).body(body.to_vec()).send().await.map_err(|_|Error::OutcomeUnknown)?;
            if response.status().as_u16()==409{return Err(Error::ResourceExhausted);}
            if !response.status().is_success(){return Err(Error::CapabilityUnavailable);}
            for (key,value) in [("x-sample-rate","24000"),("x-audio-channels","1"),("x-audio-encoding","pcm_s16le"),("x-model-stream","true")] {
                if response.headers().get(key).and_then(|v|v.to_str().ok())!=Some(value){return Err(Error::ProtocolError);}
            }
            let identifier = |name| -> Result<String,Error> {
                let value=response.headers().get(name).and_then(|v|v.to_str().ok()).ok_or(Error::ProtocolError)?;
                if value.len()!=32 || !value.bytes().all(|b|b.is_ascii_digit() || (b'a'..=b'f').contains(&b)){return Err(Error::ProtocolError);}
                Ok(value.to_owned())
            };
            let request_id=identifier("x-nai-request")?;
            let instance_id=identifier("x-nai-instance")?;
            let mut digest=Sha256::new();
            let mut bytes=response.bytes_stream(); let mut pending=Vec::new(); let mut samples=0u64; let mut first=None;
            while let Some(next)=bytes.next().await {
                operation.check()?; let next=next.map_err(|_|Error::OutcomeUnknown)?;
                if next.len()>1048576 {return Err(Error::ResourceExhausted);}
                pending.extend_from_slice(&next);
                let even=pending.len()/2*2;
                for part in pending[..even].chunks(9600) {
                    samples=samples.checked_add((part.len()/2) as u64).ok_or(Error::ResourceExhausted)?;
                    if samples>config.max_samples{return Err(Error::ResourceExhausted);}
                    operation.check()?; digest.update(part); first.get_or_insert(started.elapsed().as_millis()); accept(part)?;
                }
                pending.drain(..even);
            }
            operation.check()?;
            if !pending.is_empty() || samples==0{return Err(Error::ProtocolError);}
            let transport_eof_ms=started.elapsed().as_millis();
            // Query on the same configured origin. Server paths/redirects never select an external target.
            let mut status_url=url.clone();
            status_url.set_path(&format!("/nai/tts/status/{request_id}"));
            let status=client.get(status_url).send().await.map_err(|_|Error::OutcomeUnknown)?;
            if !status.status().is_success(){return Err(Error::ProtocolError);}
            let mut status_bytes=status.bytes_stream();
            let mut receipt_bytes=Vec::new();
            while let Some(chunk)=status_bytes.next().await {
                operation.check()?;
                let chunk=chunk.map_err(|_|Error::OutcomeUnknown)?;
                if chunk.len()>4096-receipt_bytes.len(){return Err(Error::ResourceExhausted);}
                receipt_bytes.extend_from_slice(&chunk);
            }
            let receipt: nai_contracts::cosyvoice_http::Completion=serde_json::from_slice(&receipt_bytes).map_err(|_|Error::ProtocolError)?;
            let expected_hash=digest.finalize().iter().map(|b|format!("{b:02x}")).collect::<String>();
            if receipt.protocol!="nai.cosyvoice.completion.v1" || receipt.request_id!=request_id || receipt.instance_id!=instance_id || receipt.state!="completed" || receipt.samples!=samples || receipt.sha256!=expected_hash {return Err(Error::ProtocolError);}
            operation.check()?;
            Ok(Summary{samples,first_pcm_ms:first.ok_or(Error::ProtocolError)?,transport_eof_ms,complete_ms:started.elapsed().as_millis()})
        };
        tokio::pin!(future);
        loop { tokio::select! {result=&mut future=>return result,_=tokio::time::sleep(Duration::from_millis(10))=>{operation.check()?;} } }
    })
}
