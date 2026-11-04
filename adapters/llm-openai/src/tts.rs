//! Internal static speech transport; no store, budget dispatch permit, IPC or device.
use crate::endpoint as remote_endpoint;
use futures_util::StreamExt;
use nai_contracts::workbench::WorkbenchError as Error;
use nai_runtime_core::foundation::Operation;
use nai_runtime_core::tts::{MAX_AUDIO_BYTES, MAX_SAMPLES, SAMPLE_RATE, characters};
use std::{
    sync::atomic::{AtomicU8, Ordering},
    time::Duration,
};
pub struct Request {
    pub protocol: String,
    pub language: String,
    pub base_url: String,
    pub model: String,
    pub voice: String,
    pub input: String,
    pub api_key: zeroize::Zeroizing<Vec<u8>>,
    pub deadline_ms: u32,
    pub trace_id: String,
    pub direct: bool,
}
pub fn profile_endpoint(
    value: &str,
    protocol: &str,
    language: &str,
    allow_loopback: bool,
) -> Result<reqwest::Url, Error> {
    if nai_runtime_core::tts::validate_profile(protocol, value, language)? {
        reqwest::Url::parse(value).map_err(|_| Error::InvalidArgument)
    } else {
        remote_endpoint(value, allow_loopback)
    }
}
pub struct Outcome {
    pub samples: Vec<i16>,
    pub sample_rate: u32,
    pub request_started: bool,
    pub error: Option<Error>,
    pub http_status: Option<u16>,
    pub failure_code: String,
}
/// Strict RIFF PCM decoding; response headers alone cannot establish audio format.
pub fn decode_wav(bytes: &[u8]) -> Result<Vec<i16>, Error> {
    let invalid = Error::ProtocolError;
    if bytes.len() > MAX_AUDIO_BYTES {
        return Err(Error::ResourceExhausted);
    }
    let u16le = |at: usize| -> Result<u16, Error> {
        Ok(u16::from_le_bytes(
            bytes
                .get(at..at + 2)
                .ok_or(invalid)?
                .try_into()
                .map_err(|_| invalid)?,
        ))
    };
    let u32le = |at: usize| -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            bytes
                .get(at..at + 4)
                .ok_or(invalid)?
                .try_into()
                .map_err(|_| invalid)?,
        ))
    };
    if bytes.get(..4) != Some(b"RIFF")
        || bytes.get(8..12) != Some(b"WAVE")
        || u32le(4)? as u64 + 8 != bytes.len() as u64
    {
        return Err(invalid);
    }
    let mut pos = 12usize;
    let mut format = false;
    let mut data = None;
    while pos < bytes.len() {
        let tag = bytes.get(pos..pos + 4).ok_or(invalid)?;
        let len = u32le(pos + 4)? as usize;
        let start = pos.checked_add(8).ok_or(invalid)?;
        let end = start.checked_add(len).ok_or(invalid)?;
        let chunk = bytes.get(start..end).ok_or(invalid)?;
        match tag {
            b"fmt " => {
                if format
                    || len != 16
                    || u16le(start)? != 1
                    || u16le(start + 2)? != 1
                    || u32le(start + 4)? != SAMPLE_RATE
                    || u32le(start + 8)? != SAMPLE_RATE * 2
                    || u16le(start + 12)? != 2
                    || u16le(start + 14)? != 16
                {
                    return Err(invalid);
                }
                format = true;
            }
            b"data" => {
                if data.is_some() || chunk.is_empty() || !len.is_multiple_of(2) {
                    return Err(invalid);
                }
                if len / 2 > MAX_SAMPLES {
                    return Err(Error::ResourceExhausted);
                }
                data = Some(chunk);
            }
            _ => (),
        }
        pos = end.checked_add(len % 2).ok_or(invalid)?;
        if pos > bytes.len() {
            return Err(invalid);
        }
    }
    if !format {
        return Err(invalid);
    }
    Ok(data
        .ok_or(invalid)?
        .chunks_exact(2)
        .map(|x| i16::from_le_bytes([x[0], x[1]]))
        .collect())
}
pub fn run(request: Request, cancel: Operation, allow_loopback: bool) -> Outcome {
    let mut out = Outcome {
        samples: Vec::new(),
        sample_rate: SAMPLE_RATE,
        request_started: false,
        error: None,
        http_status: None,
        failure_code: String::new(),
    };
    let execute = (|| -> Result<Vec<i16>, Error> {
        characters(&request.input)?;
        let local = nai_runtime_core::tts::validate_profile(
            &request.protocol,
            &request.base_url,
            &request.language,
        )?;
        if !request.trace_id.is_empty() {
            nai_runtime_core::foundation::uuid(&request.trace_id)?;
        }
        if request.deadline_ms == 0
            || request.deadline_ms > nai_runtime_core::tts::MODEL_TEST_DEADLINE_MS
            || [&request.model, &request.voice]
                .iter()
                .any(|s| s.trim().is_empty() || s.len() > 128 || s.chars().any(char::is_control))
            || (!local && request.api_key.is_empty())
            || (local && !request.api_key.is_empty())
            || request.api_key.len() > 4096
        {
            return Err(Error::InvalidArgument);
        }
        let auth = if local {
            None
        } else {
            let secret =
                std::str::from_utf8(&request.api_key).map_err(|_| Error::InvalidArgument)?;
            let mut auth = reqwest::header::HeaderValue::from_str(&format!("Bearer {secret}"))
                .map_err(|_| Error::InvalidArgument)?;
            auth.set_sensitive(true);
            Some(auth)
        };
        let mut url = profile_endpoint(
            &request.base_url,
            &request.protocol,
            &request.language,
            allow_loopback,
        )?;
        url.set_path(&format!(
            "{}/audio/speech",
            url.path().trim_end_matches('/')
        ));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::CapabilityUnavailable)?;
        runtime.block_on(async {
            cancel.check()?;
            let network_ms=cancel.remaining_ms()?.min(request.deadline_ms);
            let builder=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).retry(reqwest::retry::never())
                .connect_timeout(Duration::from_secs(3)).timeout(Duration::from_millis(network_ms.into()));
            let builder=if local || request.direct || allow_loopback && url.scheme()=="http" {builder.no_proxy()} else {builder};
            let client=builder.build().map_err(|_|Error::CapabilityUnavailable)?;
            let trace_enabled=!local&&!request.trace_id.is_empty()&&(url.host_str()==Some("api.siliconflow.cn")||allow_loopback);
            let stage=AtomicU8::new(0);
            let network=async {
                // Conservative: send attempt may have reached provider even without a response.
                out.request_started=true;
                let body=if local{serde_json::json!({"model":request.model,"voice":request.voice,"input":request.input,"language":request.language,"response_format":"wav"})}else{serde_json::json!({"model":request.model,"voice":request.voice,"input":request.input,"response_format":"wav","sample_rate":SAMPLE_RATE,"stream":false})};
                let request_builder=client.post(url).json(&body);
                let request_builder=if let Some(auth)=auth{request_builder.header(reqwest::header::AUTHORIZATION,auth)}else{request_builder};
                let request_builder=if trace_enabled{request_builder.header("X-Trace-Id",&request.trace_id)}else{request_builder};
                let response=request_builder.send().await.map_err(|e|if e.is_timeout(){out.failure_code=if e.is_connect(){"CONNECT_TIMEOUT"}else{"RESPONSE_TIMEOUT"}.into();Error::DeadlineExceeded}else{if e.is_connect(){out.failure_code="CONNECTION_FAILED".into();}Error::OutcomeUnknown})?;
                if !response.status().is_success() {out.http_status=Some(response.status().as_u16());out.failure_code=format!("HTTP_{}",response.status().as_u16());out.failure_code=crate::failure::response(response).await;return Err(Error::OutcomeUnknown);}
                stage.store(1,Ordering::Release);
                if response.content_length().is_some_and(|n| n>MAX_AUDIO_BYTES as u64) {return Err(Error::ResourceExhausted);}
                let mut bytes=Vec::new();let mut stream=response.bytes_stream();
                while let Some(part)=stream.next().await {
                    let part=part.map_err(|e|if e.is_timeout(){Error::DeadlineExceeded}else{Error::OutcomeUnknown})?;
                    if bytes.len().saturating_add(part.len())>MAX_AUDIO_BYTES {return Err(Error::ResourceExhausted);}
                    bytes.extend_from_slice(&part);
                }
                decode_wav(&bytes)
            };
            let result=tokio::select! {
                result=network=>result,
                e=async {loop {if let Err(e)=cancel.check() {break e;} tokio::time::sleep(Duration::from_millis(20)).await;}}=>Err(e),
                _=tokio::time::sleep(Duration::from_millis(network_ms.into()))=>Err(Error::DeadlineExceeded),
            };
            if result==Err(Error::DeadlineExceeded)&&out.failure_code.is_empty(){out.failure_code=timeout_code(stage.load(Ordering::Acquire)).into();}
            result
        })
    })();
    match execute {
        Ok(samples) => match cancel.check() {
            Ok(()) => out.samples = samples,
            Err(e) => out.error = Some(e),
        },
        Err(e) => out.error = Some(e),
    }
    out
}
fn timeout_code(stage: u8) -> &'static str {
    if stage == 0 {
        "RESPONSE_TIMEOUT"
    } else {
        "AUDIO_DOWNLOAD_TIMEOUT"
    }
}
