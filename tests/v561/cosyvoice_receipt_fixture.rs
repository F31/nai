//! Shared silent-fixture completion endpoint, never used by production.
pub fn serve(listener: &std::net::TcpListener, samples: u64, hash: &str) {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};
    listener.set_nonblocking(true).unwrap();
    let end = Instant::now() + Duration::from_secs(3);
    let (mut stream, _) = loop {
        match listener.accept() {
            Ok(value) => break value,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < end, "completion query not received");
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => panic!("{e}"),
        }
    };
    listener.set_nonblocking(false).unwrap();
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut headers = Vec::new();
    while !headers.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        headers.push(byte[0]);
        assert!(headers.len() < 16384);
    }
    assert!(headers.starts_with(b"GET /nai/tts/status/11111111111111111111111111111111 "));
    let body = serde_json::to_vec(&serde_json::json!({
        "protocol":"nai.cosyvoice.completion.v1", "request_id":"11111111111111111111111111111111",
        "instance_id":"22222222222222222222222222222222", "state":"completed", "samples":samples,"sha256":hash
    })).unwrap();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.write_all(&body).unwrap();
}
