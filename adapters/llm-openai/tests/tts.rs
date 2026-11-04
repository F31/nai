use nai_contracts::workbench::WorkbenchError as Error;
use nai_llm_openai::tts::{Request, decode_wav, run};
use nai_runtime_core::foundation::Operation;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
fn wav(samples: &[i16]) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(b"RIFF");
    b.extend((36 + samples.len() as u32 * 2).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(24000u32.to_le_bytes());
    b.extend(48000u32.to_le_bytes());
    b.extend(2u16.to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend((samples.len() as u32 * 2).to_le_bytes());
    for n in samples {
        b.extend(n.to_le_bytes());
    }
    b
}
fn request(base_url: String) -> Request {
    Request {
        protocol: "openai_speech".into(),
        language: String::new(),
        base_url,
        model: "FunAudioLLM/CosyVoice2-0.5B".into(),
        voice: "FunAudioLLM/CosyVoice2-0.5B:alex".into(),
        input: "这是隔离语音测试。".into(),
        api_key: zeroize::Zeroizing::new(b"synthetic-tts-key".to_vec()),
        deadline_ms: 2000,
        trace_id: String::new(),
        direct: false,
    }
}
#[test]
fn validates_audio_independently_of_http_headers() {
    let valid = wav(&[-32768, 0, 32767]);
    assert_eq!(decode_wav(&valid).unwrap(), vec![-32768, 0, 32767]);
    for (offset, byte) in [
        (0, b'X'),
        (4, 0),
        (20, 3),
        (22, 2),
        (24, 1),
        (28, 1),
        (32, 4),
        (34, 8),
        (40, 3),
    ] {
        let mut b = valid.clone();
        b[offset] = byte;
        assert!(decode_wav(&b).is_err(), "offset {offset}");
    }
    for len in 0..valid.len() {
        assert!(decode_wav(&valid[..len]).is_err());
    }
    assert!(decode_wav(&wav(&[])).is_err());
    assert_eq!(
        decode_wav(&wav(&vec![0; 480001])),
        Err(Error::ResourceExhausted)
    );
    let mut b = valid.clone();
    b.extend(&valid[12..]);
    let len = b.len() as u32 - 8;
    b[4..8].copy_from_slice(&len.to_le_bytes());
    assert!(decode_wav(&b).is_err());
}
#[test]
fn explicit_local_qwen_production_mode_needs_no_key_and_sends_language_without_auth() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", server.server_addr());
    let worker = std::thread::spawn(move || {
        let mut r = server
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap();
        assert_eq!(r.url(), "/v1/audio/speech");
        assert!(
            !r.headers()
                .iter()
                .any(|h| h.field.equiv("Authorization") || h.field.equiv("Proxy-Authorization"))
        );
        let mut b = String::new();
        r.as_reader().read_to_string(&mut b).unwrap();
        let v: serde_json::Value = serde_json::from_str(&b).unwrap();
        assert_eq!(v["voice"], "serena");
        assert_eq!(v["language"], "chinese");
        assert_eq!(v["response_format"], "wav");
        assert!(v.get("sample_rate").is_none());
        r.respond(tiny_http::Response::from_data(wav(&[1, 2, 3])))
            .unwrap();
        assert!(
            server
                .recv_timeout(Duration::from_millis(50))
                .unwrap()
                .is_none()
        );
    });
    let mut r = request(url);
    r.protocol = "qwen_local".into();
    r.language = "chinese".into();
    r.voice = "serena".into();
    r.api_key.clear();
    let out = run(r, Operation::model_task(45000).unwrap(), false);
    worker.join().unwrap();
    assert_eq!(out.error, None);
    assert_eq!(out.samples, vec![1, 2, 3]);
}
#[test]
fn local_profile_rejects_public_routes_and_credentials_before_any_send() {
    for mode in 0..4 {
        let mut r = request("http://127.0.0.1:8000/v1".into());
        r.protocol = "qwen_local".into();
        r.language = "chinese".into();
        r.api_key.clear();
        match mode {
            0 => r.base_url = "http://example.invalid:8000/v1".into(),
            1 => r.api_key = b"synthetic-must-not-leak".to_vec().into(),
            2 => r.language = "unknown".into(),
            _ => r.protocol = "openai_speech".into(),
        };
        let out = run(r, Operation::model_task(45000).unwrap(), false);
        assert!(!out.request_started);
        assert!(out.error.is_some());
    }
}
#[test]
fn real_http_uses_explicit_format_and_returns_owned_audio_once() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", server.server_addr());
    let worker =
        std::thread::spawn(move || {
            let mut r = server
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .unwrap();
            assert_eq!(r.url(), "/v1/audio/speech");
            assert_eq!(r.method(), &tiny_http::Method::Post);
            assert!(r.headers().iter().any(|h| h.field.equiv("X-Trace-Id")
                && h.value.as_str() == "00000000-0000-4000-8000-000000000001"));
            assert!(r.headers().iter().any(|h| h.field.equiv("Authorization")
                && h.value.as_str() == "Bearer synthetic-tts-key"));
            let mut body = String::new();
            r.as_reader().read_to_string(&mut body).unwrap();
            let v: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(v["response_format"], "wav");
            assert_eq!(v["sample_rate"], 24000);
            assert_eq!(v["stream"], false);
            assert_eq!(v["voice"], "FunAudioLLM/CosyVoice2-0.5B:alex");
            r.respond(tiny_http::Response::from_data(wav(&[1, -2, 3])))
                .unwrap();
            assert!(
                server
                    .recv_timeout(Duration::from_millis(100))
                    .unwrap()
                    .is_none()
            );
        });
    let mut req = request(url);
    req.deadline_ms = 45_000;
    req.trace_id = "00000000-0000-4000-8000-000000000001".into();
    let result = run(req, Operation::model_task(45000).unwrap(), true);
    assert_eq!(result.error, None);
    assert!(result.request_started);
    assert_eq!(result.samples, vec![1, -2, 3]);
    assert_eq!(result.sample_rate, 24000);
    worker.join().unwrap();
}
#[test]
fn failure_never_returns_error_body_or_partial_audio_and_does_not_redirect() {
    for (status, body, expected) in [
        (
            401,
            b"synthetic-tts-key raw provider error".to_vec(),
            Error::OutcomeUnknown,
        ),
        (200, b"not audio".to_vec(), Error::ProtocolError),
        (302, wav(&[1]), Error::OutcomeUnknown),
        (200, vec![0; 1_048_577], Error::ResourceExhausted),
    ] {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", server.server_addr());
        let worker = std::thread::spawn(move || {
            let r = server
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .unwrap();
            let _ = r.respond(
                tiny_http::Response::from_data(body)
                    .with_status_code(status)
                    .with_header(
                        tiny_http::Header::from_bytes("Location", "http://127.0.0.1:1/forbidden")
                            .unwrap(),
                    ),
            );
            assert!(
                server
                    .recv_timeout(Duration::from_millis(100))
                    .unwrap()
                    .is_none()
            );
        });
        let mut req = request(url);
        req.deadline_ms = 45_000;
        req.trace_id = "00000000-0000-4000-8000-000000000001".into();
        let r = run(req, Operation::model_task(45000).unwrap(), true);
        assert_eq!(r.error, Some(expected));
        assert!(r.request_started);
        assert!(r.samples.is_empty());
        if status != 200 {
            assert_eq!(r.http_status, Some(status));
        }
        worker.join().unwrap();
    }
}
#[test]
fn cancel_before_dispatch_is_free_and_cancel_after_dispatch_discards_audio() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", server.server_addr());
    let flag = Operation::model_task(45000).unwrap();
    flag.cancel();
    let result = run(request(url.clone()), flag.clone(), true);
    assert!(!result.request_started);
    assert_eq!(result.error, Some(Error::Cancelled));
    assert!(
        server
            .recv_timeout(Duration::from_millis(50))
            .unwrap()
            .is_none()
    );
    let flag = Operation::model_task(45000).unwrap();
    let (tx, rx) = mpsc::sync_channel(1);
    let (finish, gate) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        let r = server
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap();
        tx.send(()).unwrap();
        gate.recv_timeout(Duration::from_secs(3)).unwrap();
        let _ = r.respond(tiny_http::Response::from_data(wav(&[1, 2])));
    });
    let network_flag = flag.model_child(45000).unwrap();
    let run = std::thread::spawn(move || run(request(url), network_flag, true));
    rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let time = Instant::now();
    flag.cancel();
    let r = run.join().unwrap();
    assert!(time.elapsed() < Duration::from_secs(1));
    assert_eq!(r.error, Some(Error::Cancelled));
    assert!(r.request_started);
    assert!(r.samples.is_empty());
    finish.send(()).unwrap();
    worker.join().unwrap();
}
#[test]
fn deadline_bounds_silent_server_and_invalid_configuration_never_dispatches() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", server.server_addr());
    let (tx, rx) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        let r = server
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap();
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
        drop(r);
    });
    let mut req = request(url.clone());
    req.deadline_ms = 100;
    let now = Instant::now();
    let r = run(req, Operation::model_task(45000).unwrap(), true);
    assert_eq!(r.error, Some(Error::DeadlineExceeded));
    assert_eq!(r.failure_code, "RESPONSE_TIMEOUT");
    assert!(now.elapsed() < Duration::from_secs(1));
    assert!(r.samples.is_empty());
    tx.send(()).unwrap();
    worker.join().unwrap();
    for mode in 0..4 {
        let mut r = request(url.clone());
        match mode {
            0 => r.deadline_ms = 45001,
            1 => r.input = "中".repeat(257),
            2 => r.api_key.clear(),
            _ => r.voice = "bad\nvoice".into(),
        };
        let r = run(r, Operation::model_task(45000).unwrap(), true);
        assert!(!r.request_started);
        assert!(r.samples.is_empty());
        assert!(r.error.is_some());
    }
    let r = run(request(url), Operation::model_task(45000).unwrap(), false);
    assert!(!r.request_started);
    assert_eq!(r.error, Some(Error::InvalidArgument));
}
#[test]
fn successful_headers_with_stalled_audio_report_download_timeout_and_never_return_partial_audio() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let (release, wait) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut header = Vec::new();
        let mut byte = [0; 1];
        while !header.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            header.push(byte[0]);
            assert!(header.len() < 8192);
        }
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nContent-Length: 100\r\nConnection: close\r\n\r\nRIFF").unwrap();
        socket.flush().unwrap();
        let _ = wait.recv_timeout(Duration::from_secs(3));
    });
    let mut req = request(url);
    req.deadline_ms = 500;
    let start = Instant::now();
    let result = run(req, Operation::model_task(45000).unwrap(), true);
    release.send(()).unwrap();
    worker.join().unwrap();
    assert_eq!(result.error, Some(Error::DeadlineExceeded));
    assert_eq!(result.failure_code, "AUDIO_DOWNLOAD_TIMEOUT");
    assert!(result.request_started);
    assert!(result.samples.is_empty());
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn short_parent_deadline_prevents_a_late_speech_child_from_returning_audio() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", server.server_addr());
    let (arrived, wait) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let service = std::thread::spawn(move || {
        let r = server
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap();
        arrived.send(()).unwrap();
        gate.recv_timeout(Duration::from_secs(3)).unwrap();
        let _ = r.respond(tiny_http::Response::from_data(wav(&[1, 2])));
        assert!(
            server
                .recv_timeout(Duration::from_millis(50))
                .unwrap()
                .is_none()
        );
    });
    let parent = Operation::model_task(250).unwrap();
    let child = parent.model_child(45000).unwrap();
    let start = Instant::now();
    let worker = std::thread::spawn(move || run(request(url), child, true));
    wait.recv_timeout(Duration::from_secs(2)).unwrap();
    let out = worker.join().unwrap();
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(out.request_started);
    assert!(out.samples.is_empty());
    assert_eq!(out.error, Some(Error::DeadlineExceeded));
    assert_eq!(out.failure_code, "RESPONSE_TIMEOUT");
    release.send(()).unwrap();
    service.join().unwrap();
}
