use nai_contracts::workbench::WorkbenchError as Error;
use nai_llm_openai::cosyvoice::{self, Config};
use nai_runtime_core::foundation::Operation;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time::Duration,
};

fn request(stream: &mut TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    let mut length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        if let Some((key, value)) = line.split_once(':')
            && key.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap();
        }
    }
    assert!(length < 32768);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    assert!(String::from_utf8_lossy(&body).contains("<|endofprompt|>"));
}
fn server(f: impl FnOnce(TcpStream) + Send + 'static) -> (String, std::thread::JoinHandle<()>) {
    server_receipt(f, None)
}
fn server_receipt(
    f: impl FnOnce(TcpStream) + Send + 'static,
    change: Option<(&'static str, serde_json::Value)>,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "http://{}/inference_zero_shot",
        listener.local_addr().unwrap()
    );
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        request(&mut stream);
        f(stream);
        listener.set_nonblocking(true).unwrap();
        let end = std::time::Instant::now() + Duration::from_millis(300);
        loop {
            match listener.accept() {
                Ok((mut s, _)) => {
                    s.set_nonblocking(false).unwrap();
                    s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                    let mut line = String::new();
                    BufReader::new(&mut s).read_line(&mut line).unwrap();
                    assert!(
                        line.starts_with("GET /nai/tts/status/11111111111111111111111111111111 ")
                    );
                    use sha2::{Digest, Sha256};
                    let hash = Sha256::digest([1, 0, 2, 0, 3, 0])
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>();
                    let mut receipt = serde_json::json!({"protocol":"nai.cosyvoice.completion.v1","request_id":"11111111111111111111111111111111","instance_id":"22222222222222222222222222222222","state":"completed","samples":3,"sha256":hash});
                    if let Some((key, value)) = change {
                        if key == "__status" {
                            s.write_all(b"HTTP/1.1 404 Not Found\r\nConnection: close\r\nContent-Length: 0\r\n\r\n").unwrap();
                            break;
                        }
                        receipt[key] = value;
                    }
                    let bytes = serde_json::to_vec(&receipt).unwrap();
                    write!(
                        s,
                        "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                        bytes.len()
                    )
                    .unwrap();
                    s.write_all(&bytes).unwrap();
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if std::time::Instant::now() >= end {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => panic!("{e}"),
            }
        }
    });
    (url, thread)
}
fn headers(stream: &mut TcpStream, encoding: &str) {
    write!(stream,"HTTP/1.1 200 OK\r\nConnection: close\r\nTransfer-Encoding: chunked\r\nContent-Type: application/octet-stream\r\nX-Sample-Rate: 24000\r\nX-Audio-Channels: 1\r\nX-Audio-Encoding: {encoding}\r\nX-Model-Stream: true\r\nX-Nai-Request: 11111111111111111111111111111111\r\nX-Nai-Instance: 22222222222222222222222222222222\r\n\r\n").unwrap();
    stream.flush().unwrap();
}
fn part(stream: &mut TcpStream, bytes: &[u8]) {
    write!(stream, "{:x}\r\n", bytes.len()).unwrap();
    stream.write_all(bytes).unwrap();
    stream.write_all(b"\r\n").unwrap();
    stream.flush().unwrap();
}
fn config(url: String, file: &tempfile::NamedTempFile) -> Config {
    Config {
        endpoint: url,
        reference_file: file.path().to_path_buf(),
        prompt_text: "test<|endofprompt|>prompt".into(),
        deadline_ms: 2000,
        max_samples: 480000,
        lookahead: Default::default(),
    }
}
fn fixture() -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(b"reference fixture").unwrap();
    f
}

#[test]
fn odd_network_boundaries_are_joined_and_first_pcm_is_committed_before_eof() {
    let (tx, rx) = mpsc::channel();
    let (url, thread) = server(move |mut s| {
        headers(&mut s, "pcm_s16le");
        part(&mut s, &[1]);
        part(&mut s, &[0, 2, 0]);
        rx.recv_timeout(Duration::from_secs(1))
            .expect("adapter must emit without EOF");
        part(&mut s, &[3, 0]);
        s.write_all(b"0\r\n\r\n").unwrap();
    });
    let file = fixture();
    let mut data = vec![];
    let summary = cosyvoice::stream(
        &config(url, &file),
        "test",
        Operation::model_task(2000).unwrap(),
        |bytes| {
            data.extend_from_slice(bytes);
            let _ = tx.send(());
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(data, [1, 0, 2, 0, 3, 0]);
    assert_eq!(summary.samples, 3);
    thread.join().unwrap();
}
#[test]
fn malformed_encoding_and_odd_eof_are_not_successful_audio() {
    for valid_header in [true, false] {
        let (url, thread) = server(move |mut s| {
            headers(&mut s, if valid_header { "pcm_s16le" } else { "wav" });
            part(&mut s, &[1]);
            let _ = s.write_all(b"0\r\n\r\n");
        });
        let file = fixture();
        let result = cosyvoice::stream(
            &config(url, &file),
            "test",
            Operation::model_task(2000).unwrap(),
            |_| panic!("no complete PCM was sent"),
        );
        assert!(matches!(result, Err(Error::ProtocolError)));
        thread.join().unwrap();
    }
}
#[test]
fn parent_cancel_interrupts_waiting_headers() {
    let (tx, rx) = mpsc::channel();
    let (url, thread) = server(move |_s| {
        tx.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(200));
    });
    let file = fixture();
    let op = Operation::model_task(2000).unwrap();
    let parent = op.clone();
    let cancel = std::thread::spawn(move || {
        rx.recv().unwrap();
        parent.cancel();
    });
    let started = std::time::Instant::now();
    assert!(matches!(
        cosyvoice::stream(&config(url, &file), "test", op, |_| Ok(())),
        Err(Error::Cancelled)
    ));
    assert!(started.elapsed() < Duration::from_millis(180));
    cancel.join().unwrap();
    thread.join().unwrap();
}
#[test]
fn sample_limit_and_busy_are_not_retried() {
    for busy in [false, true] {
        let (url, thread) = server(move |mut s| {
            if busy {
                s.write_all(b"HTTP/1.1 409 Conflict\r\nContent-Length: 0\r\n\r\n")
                    .unwrap();
            } else {
                headers(&mut s, "pcm_s16le");
                part(&mut s, &[1, 0, 2, 0]);
                let _ = s.write_all(b"0\r\n\r\n");
            }
        });
        let file = fixture();
        let mut c = config(url, &file);
        c.max_samples = 1;
        let result = cosyvoice::stream(&c, "test", Operation::model_task(2000).unwrap(), |_| {
            panic!("must reject before output")
        });
        assert!(matches!(
            result,
            Err(Error::CapabilityUnavailable) | Err(Error::ResourceExhausted)
        ));
        thread.join().unwrap();
    }
}

#[test]
fn normal_http_eof_requires_matching_completed_application_receipt() {
    for (field, value) in [
        ("state", serde_json::json!("failed")),
        ("state", serde_json::json!("running")),
        ("samples", serde_json::json!(2)),
        ("sha256", serde_json::json!("0".repeat(64))),
        ("instance_id", serde_json::json!("3".repeat(32))),
        ("request_id", serde_json::json!("4".repeat(32))),
        ("protocol", serde_json::json!("unknown")),
        ("extra", serde_json::json!(true)),
        ("__status", serde_json::json!(404)),
    ] {
        let (url, thread) = server_receipt(
            |mut s| {
                headers(&mut s, "pcm_s16le");
                part(&mut s, &[1, 0, 2, 0, 3, 0]);
                s.write_all(b"0\r\n\r\n").unwrap();
            },
            Some((field, value)),
        );
        let file = fixture();
        assert!(matches!(
            cosyvoice::stream(
                &config(url, &file),
                "test",
                Operation::model_task(2000).unwrap(),
                |_| Ok(())
            ),
            Err(Error::ProtocolError)
        ));
        thread.join().unwrap();
    }
}

#[test]
fn completion_response_is_bounded_and_missing_identity_is_not_accepted() {
    let (url, thread) = server_receipt(
        |mut s| {
            headers(&mut s, "pcm_s16le");
            part(&mut s, &[1, 0, 2, 0, 3, 0]);
            s.write_all(b"0\r\n\r\n").unwrap();
        },
        Some(("extra", serde_json::json!("x".repeat(5000)))),
    );
    let file = fixture();
    assert!(matches!(
        cosyvoice::stream(
            &config(url, &file),
            "test",
            Operation::model_task(2000).unwrap(),
            |_| Ok(())
        ),
        Err(Error::ResourceExhausted)
    ));
    thread.join().unwrap();
    let (url, thread) = server(|mut s| {
        s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nX-Sample-Rate: 24000\r\nX-Audio-Channels: 1\r\nX-Audio-Encoding: pcm_s16le\r\nX-Model-Stream: true\r\n\r\n\0\0").unwrap();
    });
    assert!(matches!(
        cosyvoice::stream(
            &config(url, &file),
            "test",
            Operation::model_task(2000).unwrap(),
            |_| panic!("missing completion identity must reject before PCM")
        ),
        Err(Error::ProtocolError)
    ));
    thread.join().unwrap();
}

#[test]
fn parent_cancel_interrupts_waiting_completion_receipt() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "http://{}/inference_zero_shot",
        listener.local_addr().unwrap()
    );
    let (tx, rx) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        request(&mut s);
        headers(&mut s, "pcm_s16le");
        part(&mut s, &[1, 0, 2, 0, 3, 0]);
        s.write_all(b"0\r\n\r\n").unwrap();
        drop(s);
        let (mut s, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(&mut s).read_line(&mut line).unwrap();
        assert!(line.starts_with("GET /nai/tts/status/"));
        tx.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(250));
    });
    let operation = Operation::model_task(2000).unwrap();
    let parent = operation.clone();
    let cancel = std::thread::spawn(move || {
        rx.recv_timeout(Duration::from_secs(1)).unwrap();
        parent.cancel();
    });
    let file = fixture();
    let started = std::time::Instant::now();
    assert!(matches!(
        cosyvoice::stream(&config(url, &file), "test", operation, |_| Ok(())),
        Err(Error::Cancelled)
    ));
    assert!(started.elapsed() < Duration::from_millis(200));
    cancel.join().unwrap();
    thread.join().unwrap();
}

#[test]
fn drain_observation_only_polls_health_and_remains_cancellable() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    };
    for cancel in [false, true] {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let file = fixture();
        let c = config(
            format!("http://{}/inference_zero_shot", server.server_addr()),
            &file,
        );
        let stop = Arc::new(AtomicBool::new(false));
        let count = Arc::new(AtomicU64::new(0));
        let (done, seen) = (stop.clone(), count.clone());
        let worker = std::thread::spawn(move || {
            while !done.load(Ordering::Acquire) {
                let Some(request) = server.recv_timeout(Duration::from_millis(10)).unwrap() else {
                    continue;
                };
                assert_eq!(request.method(), &tiny_http::Method::Get);
                assert_eq!(request.url(), "/health");
                let n = seen.fetch_add(1, Ordering::AcqRel) + 1;
                request
                    .respond(tiny_http::Response::from_string(format!(
                        "{{\"status\":\"ok\",\"busy\":{}}}",
                        cancel || n < 2
                    )))
                    .unwrap();
            }
        });
        let op = Operation::model_task(1000).unwrap();
        let parent = op.clone();
        let observed = count.clone();
        let canceller = std::thread::spawn(move || {
            if cancel {
                let end = std::time::Instant::now() + Duration::from_millis(500);
                while observed.load(Ordering::Acquire) == 0 {
                    assert!(std::time::Instant::now() < end);
                    std::thread::sleep(Duration::from_millis(2));
                }
                parent.cancel();
            }
        });
        let result = cosyvoice::wait_idle(&c, op);
        stop.store(true, Ordering::Release);
        worker.join().unwrap();
        canceller.join().unwrap();
        if cancel {
            assert!(matches!(result, Err(Error::Cancelled)));
        } else {
            assert!(result.is_ok());
            assert_eq!(count.load(Ordering::Acquire), 2);
        }
    }
}
