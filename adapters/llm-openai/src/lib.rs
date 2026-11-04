//! Trusted static HTTPS adapter. No DB, legacy ABI or automatic retry/failover.
mod failure;
pub mod vault;
use futures_util::StreamExt;
use nai_contracts::workbench::{LlmWorkerRequest, LlmWorkerResult, WorkbenchError as Error};
use nai_runtime_core::foundation::{Operation, Result};
use std::time::Duration;
pub fn endpoint(value: &str, allow_loopback: bool) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(value).map_err(|_| Error::InvalidArgument)?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host_str().is_none()
    {
        return Err(Error::InvalidArgument);
    }
    let test = allow_loopback
        && url.scheme() == "http"
        && matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "localhost"));
    if url.scheme() != "https" && !test {
        return Err(Error::InvalidArgument);
    }
    Ok(url)
}
pub fn run(
    mut request: LlmWorkerRequest,
    cancel: Operation,
    allow_loopback: bool,
) -> LlmWorkerResult {
    let id = request.task_id.clone();
    let key = zeroize::Zeroizing::new(std::mem::take(&mut request.api_key));
    let mut result = LlmWorkerResult {
        task_id: id,
        ..Default::default()
    };
    let execute = (|| -> Result<()> {
        let mut url = endpoint(&request.base_url, allow_loopback)?;
        url.set_path(&format!(
            "{}/chat/completions",
            url.path().trim_end_matches('/')
        ));
        if request.deadline_ms == 0
            || request.deadline_ms > 15000
            || request.max_output_tokens != 256
            || request.input.len() > 4096
            || key.is_empty()
            || key.len() > 4096
        {
            return Err(Error::InvalidArgument);
        }
        let secret = std::str::from_utf8(&key).map_err(|_| Error::InvalidArgument)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::CapabilityUnavailable)?;
        runtime.block_on(async{
   let network_ms=cancel.remaining_ms()?.min(request.deadline_ms);
   let builder=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).retry(reqwest::retry::never()).connect_timeout(Duration::from_secs(3)).timeout(Duration::from_millis(network_ms.into()));
   let builder=if allow_loopback&&url.scheme()=="http"{builder.no_proxy()}else{builder};
   let client=builder.build().map_err(|_|Error::CapabilityUnavailable)?;
   cancel.check()?;
   let mut auth=reqwest::header::HeaderValue::from_str(&format!("Bearer {secret}")).map_err(|_|Error::InvalidArgument)?;auth.set_sensitive(true);
   let mut body=serde_json::json!({"model":request.model,"stream":false,"max_tokens":request.max_output_tokens,"response_format":{"type":"json_object"},"messages":[{"role":"system","content":request.system},{"role":"user","content":request.input}]});
   if request.connection_test { body.as_object_mut().ok_or(Error::ProtocolError)?.remove("response_format"); }
   result.request_started=true;
   let network=async{
    let response=client.post(url).header(reqwest::header::AUTHORIZATION,auth).json(&body).send().await.map_err(|e|if e.is_timeout(){Error::DeadlineExceeded}else{Error::OutcomeUnknown})?;
    if !response.status().is_success(){result.error=failure::response(response).await;return Err(Error::OutcomeUnknown);}
    let mut bytes=Vec::new();let mut stream=response.bytes_stream();while let Some(part)=stream.next().await{let part=part.map_err(|_|Error::OutcomeUnknown)?;if bytes.len()+part.len()>65536{return Err(Error::ResourceExhausted);}bytes.extend_from_slice(&part);}
    let value:serde_json::Value=match serde_json::from_slice(&bytes){Ok(v)=>v,Err(_)=>{result.error="RESPONSE_INVALID_JSON".into();return Err(Error::ProtocolError);}};
    if let(Some(input),Some(output))=(value["usage"]["prompt_tokens"].as_u64(),value["usage"]["completion_tokens"].as_u64())&&input<=1000000000&&output<=1000000000{result.input_tokens=input;result.output_tokens=output;result.usage_known=true;}
    if value["choices"][0]["finish_reason"]!="stop"{result.error=if value["choices"][0]["finish_reason"]=="length"{"RESPONSE_TRUNCATED"}else{"RESPONSE_FINISH_UNEXPECTED"}.into();return Err(Error::ProtocolError);}
    let text=match value["choices"][0]["message"]["content"].as_str(){Some(t) if !t.trim().is_empty()=>t,_=>{result.error="RESPONSE_EMPTY_CONTENT".into();return Err(Error::ProtocolError);}};if text.contains(secret){result.error="RESPONSE_SENSITIVE_CONTENT".into();return Err(Error::ProtocolError);}
    if text.len()>8192{return Err(Error::ResourceExhausted);}result.text=text.into();Ok(())
   };
   tokio::select!{r=network=>r,e=async{loop{if let Err(e)=cancel.check(){break e;}tokio::time::sleep(Duration::from_millis(20)).await;}}=>Err(e),_=tokio::time::sleep(Duration::from_millis(network_ms.into()))=>Err(Error::DeadlineExceeded)}
  })
    })();
    let execute = execute.and_then(|()| cancel.check());
    if let Err(error) = execute {
        if result.error.is_empty() {
            result.error = error.as_str_name().into();
        }
        result.text.clear();
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request(url: String) -> LlmWorkerRequest {
        LlmWorkerRequest {
            task_id: "test-task".into(),
            connection_test: false,
            base_url: url,
            model: "test-model".into(),
            api_key: b"synthetic-key".to_vec(),
            system: "constraints".into(),
            input: "facts".into(),
            max_output_tokens: 256,
            deadline_ms: 2000,
        }
    }
    #[test]
    fn model_probe_classifies_response_without_retry_or_json_mode() {
        for (status, body, expected) in [
            (200, serde_json::json!({"choices":[{"finish_reason":"stop","message":{"content":"NAI_OK"}}],"usage":{"prompt_tokens":8,"completion_tokens":3}}).to_string(), ""),
            (401, "do not disclose raw error or synthetic-key".into(), "HTTP_401"),
            (200, "not json".into(), "RESPONSE_INVALID_JSON"),
            (200, serde_json::json!({"choices":[{"finish_reason":"length","message":{"content":"partial"}}]}).to_string(), "RESPONSE_TRUNCATED"),
            (200, serde_json::json!({"choices":[{"finish_reason":"stop","message":{"content":""}}]}).to_string(), "RESPONSE_EMPTY_CONTENT"),
            (200, serde_json::json!({"choices":[{"finish_reason":"stop","message":{"content":"synthetic-key"}}]}).to_string(), "RESPONSE_SENSITIVE_CONTENT"),
        ] {
            let server=tiny_http::Server::http("127.0.0.1:0").unwrap();let url=format!("http://{}/v1",server.server_addr());
            let thread=std::thread::spawn(move||{let mut r=server.recv_timeout(Duration::from_secs(4)).unwrap().unwrap();let mut input=String::new();r.as_reader().read_to_string(&mut input).unwrap();let v:serde_json::Value=serde_json::from_str(&input).unwrap();assert!(v.get("response_format").is_none());assert_eq!(v["max_tokens"],256);r.respond(tiny_http::Response::from_string(body).with_status_code(status)).unwrap();assert!(server.recv_timeout(Duration::from_millis(50)).unwrap().is_none());});
            let mut r=request(url);r.connection_test=true;let reply=run(r,Operation::model_task(15000).unwrap(),true);thread.join().unwrap();assert_eq!(reply.error,expected);assert!(reply.request_started);if expected.is_empty(){assert_eq!(reply.text,"NAI_OK");assert!(reply.usage_known);}else{assert!(reply.text.is_empty());}
        }
    }
    #[test]
    fn endpoint_has_no_credentials_query_or_insecure_production_fallback() {
        for url in [
            "http://example.com/v1",
            "https://user:secret@example.com/v1",
            "https://example.com/v1?key=test",
            "https://example.com/#fragment",
        ] {
            assert!(endpoint(url, false).is_err());
        }
        assert!(endpoint("https://example.invalid/v1", false).is_ok());
        assert!(endpoint("http://127.0.0.1:1234/v1", true).is_ok());
    }
    #[test]
    fn deadline_leaves_sent_usage_unknown_and_never_retries() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let mut r = request(format!("http://{}/v1", server.server_addr()));
        r.deadline_ms = 500;
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            let _ = tx.send(run(r, Operation::model_task(15000).unwrap(), true));
        });
        let held = server
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .expect("request dispatched");
        let result = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("bounded deadline");
        assert!(result.request_started);
        assert!(!result.usage_known);
        assert!(result.text.is_empty());
        assert!(matches!(
            result.error.as_str(),
            "WORKBENCH_ERROR_DEADLINE_EXCEEDED" | "WORKBENCH_ERROR_OUTCOME_UNKNOWN"
        ));
        assert!(
            server
                .recv_timeout(Duration::from_millis(150))
                .unwrap()
                .is_none()
        );
        drop(held);
        worker.join().unwrap();
    }
    #[test]
    fn real_local_http_is_single_request_and_reports_usage_without_claiming_truth() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", server.server_addr());
        let thread = std::thread::spawn(move || {
            let mut r = server
                .recv_timeout(Duration::from_secs(4))
                .unwrap()
                .expect("test HTTP request must arrive");
            assert_eq!(r.url(), "/v1/chat/completions");
            assert!(
                r.headers().iter().any(|h| h.field.equiv("Authorization")
                    && h.value.as_str() == "Bearer synthetic-key")
            );
            let mut bytes = String::new();
            r.as_reader().read_to_string(&mut bytes).unwrap();
            let value: serde_json::Value = serde_json::from_str(&bytes).unwrap();
            assert_eq!(value["stream"], false);
            assert_eq!(value["max_tokens"], 256);
            assert_eq!(value["messages"][0]["role"], "system");
            let body=serde_json::json!({"choices":[{"finish_reason":"stop","message":{"content":"{\"text\":\"标准款500毫升\",\"fact_ids\":[\"capacity\"]}"}}],"usage":{"prompt_tokens":100,"completion_tokens":20}}).to_string();
            r.respond(tiny_http::Response::from_string(body)).unwrap();
            assert!(
                server
                    .recv_timeout(Duration::from_millis(100))
                    .unwrap()
                    .is_none()
            );
        });
        let reply = run(request(url), Operation::model_task(15000).unwrap(), true);
        thread.join().unwrap();
        assert!(reply.error.is_empty());
        assert!(reply.usage_known);
        assert_eq!(reply.input_tokens, 100);
        assert_eq!(reply.output_tokens, 20);
        assert!(reply.request_started);
    }
    #[test]
    fn cancel_returns_promptly_and_never_retries() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", server.server_addr());
        let cancel = Operation::model_task(15000).unwrap();
        let flag = cancel.clone();
        let child = cancel.model_child(15000).unwrap();
        let thread = std::thread::spawn(move || {
            let r = server
                .recv_timeout(Duration::from_secs(4))
                .unwrap()
                .expect("test HTTP request must arrive");
            flag.cancel();
            std::thread::sleep(Duration::from_millis(200));
            drop(r);
            assert!(
                server
                    .recv_timeout(Duration::from_millis(100))
                    .unwrap()
                    .is_none()
            );
        });
        let start = std::time::Instant::now();
        let reply = run(request(url), child, true);
        assert!(start.elapsed() < Duration::from_millis(1000));
        assert!(!reply.usage_known);
        assert!(reply.request_started);
        assert!(!reply.error.is_empty());
        thread.join().unwrap();
    }
    #[test]
    fn missing_usage_and_truncated_generation_are_not_complete() {
        for valid in [true, false] {
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            let url = format!("http://{}/v1", server.server_addr());
            let thread = std::thread::spawn(move || {
                server.recv_timeout(Duration::from_secs(4)).unwrap().expect("test HTTP request must arrive").respond(tiny_http::Response::from_string(serde_json::json!({"choices":[{"finish_reason":if valid{"stop"}else{"length"},"message":{"content":"{}"}}]}).to_string())).unwrap();
            });
            let reply = run(request(url), Operation::model_task(15000).unwrap(), true);
            assert!(!reply.usage_known);
            assert_eq!(reply.error.is_empty(), valid);
            thread.join().unwrap();
        }
    }
    #[test]
    fn short_parent_deadline_stops_longer_network_child_without_retry() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", server.server_addr());
        let (arrived, wait) = std::sync::mpsc::sync_channel(1);
        let (release, gate) = std::sync::mpsc::sync_channel(1);
        let service = std::thread::spawn(move || {
            let r = server
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .unwrap();
            arrived.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(3)).unwrap();
            drop(r);
            assert!(
                server
                    .recv_timeout(Duration::from_millis(50))
                    .unwrap()
                    .is_none()
            );
        });
        let parent = Operation::model_task(250).unwrap();
        let child = parent.model_child(15000).unwrap();
        let start = std::time::Instant::now();
        let worker = std::thread::spawn(move || run(request(url), child, true));
        wait.recv_timeout(Duration::from_secs(2)).unwrap();
        let reply = worker.join().unwrap();
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(reply.request_started);
        assert!(reply.text.is_empty());
        assert_eq!(reply.error, Error::DeadlineExceeded.as_str_name());
        release.send(()).unwrap();
        service.join().unwrap();
    }
}

pub mod tts;
