#![cfg(windows)]
//! Explicit native supervisor tests: local HTTP fixtures and silent device PCM.
use nai_contracts::v561_audition::{Event, Kind};
#[path = "../../../tests/v561/cosyvoice_receipt_fixture.rs"]
mod completion_fixture;
use nai_workbench_host::v561_audition::Controller;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

fn body(stream: &mut TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut headers = vec![];
    let mut byte = [0];
    while !headers.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).unwrap();
        headers.push(byte[0]);
        assert!(headers.len() < 16384);
    }
    let n = String::from_utf8(headers)
        .unwrap()
        .lines()
        .find_map(|l| {
            l.split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                .map(|(_, v)| v.trim().parse::<usize>().unwrap())
        })
        .unwrap();
    let mut bytes = vec![0; n];
    stream.read_exact(&mut bytes).unwrap();
}
fn complete(stream: &mut TcpStream) {
    let _=stream.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\nTransfer-Encoding: chunked\r\nX-Sample-Rate: 24000\r\nX-Audio-Channels: 1\r\nX-Audio-Encoding: pcm_s16le\r\nX-Model-Stream: true\r\nX-Nai-Request: 11111111111111111111111111111111\r\nX-Nai-Instance: 22222222222222222222222222222222\r\n\r\n");
    let _ = stream.write_all(b"4b00\r\n");
    let _ = stream.write_all(&vec![0; 19200]);
    let _ = stream.write_all(b"\r\n0\r\n\r\n");
}
fn config(server: &TcpListener) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let reference = dir.path().join("reference.wav");
    std::fs::write(&reference, b"fixture").unwrap();
    let path = dir.path().join("voice.json");
    std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"endpoint":format!("http://{}/inference_zero_shot",server.local_addr().unwrap()),"reference_file":reference,"prompt_text":"fixture<|endofprompt|>","deadline_ms":10000,"max_samples":480000})).unwrap()).unwrap();
    (dir, path)
}
fn controller(config: PathBuf) -> Controller {
    Controller::new(
        std::env::var_os("NAI_AUDITION_CONTROL")
            .map(PathBuf::from)
            .expect("set NAI_AUDITION_CONTROL"),
        std::env::var_os("NAI_STREAM_AUDIO_OUTPUT")
            .map(PathBuf::from)
            .expect("set NAI_STREAM_AUDIO_OUTPUT"),
        Some(config),
    )
    .unwrap()
}
fn wait(controller: &Controller, predicate: impl Fn(&Event) -> bool) -> Event {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let view = controller.status();
        if predicate(&view) {
            return view;
        }
        assert!(Instant::now() < end, "status {:?}", view);
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "explicit Windows native silent audition control"]
fn private_control_completes_and_ui_status_has_device_counts() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let (_dir, path) = config(&server);
    let http = std::thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        body(&mut stream);
        complete(&mut stream);
        drop(stream);
        completion_fixture::serve(
            &server,
            9600,
            "17744bb6a4ed1d9d51d85d7eb645cde30352530d494233c434027f9e3193bae4",
        );
    });
    let controller = controller(path);
    controller.start("fixture".into()).unwrap();
    let view = wait(&controller, |v| !v.busy);
    assert_eq!(view.kind, Kind::Completed as i32);
    assert_eq!(view.samples, Some(9600));
    assert_eq!(view.played_samples, Some(9600));
    http.join().unwrap();
}

#[test]
#[ignore = "explicit Windows native cancellation and stale-result test"]
fn stop_waiting_generation_rejects_late_completion_then_explicit_restart() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let (_dir, path) = config(&server);
    let (accepted, ready) = mpsc::channel();
    let (release, late) = mpsc::channel();
    let http = std::thread::spawn(move || {
        let (mut first, _) = server.accept().unwrap();
        body(&mut first);
        accepted.send(()).unwrap();
        late.recv_timeout(Duration::from_secs(5)).unwrap();
        complete(&mut first);
        let (mut second, _) = server.accept().unwrap();
        body(&mut second);
        complete(&mut second);
        drop(second);
        completion_fixture::serve(
            &server,
            9600,
            "17744bb6a4ed1d9d51d85d7eb645cde30352530d494233c434027f9e3193bae4",
        );
    });
    let controller = controller(path);
    controller.start("fixture".into()).unwrap();
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(controller.start("must not start".into()).is_err());
    let now = Instant::now();
    controller.stop();
    assert!(now.elapsed() < Duration::from_millis(100));
    let stopped = wait(&controller, |v| !v.busy);
    assert_eq!(stopped.kind, Kind::Cancelled as i32);
    assert!(stopped.played_samples.is_none());
    release.send(()).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(controller.status().kind, Kind::Cancelled as i32);
    controller.start("explicit restart".into()).unwrap();
    let finished = wait(&controller, |v| !v.busy);
    assert_eq!(finished.kind, Kind::Completed as i32);
    assert_eq!(finished.task_id, 2);
    http.join().unwrap();
}

#[test]
#[ignore = "explicit Windows native close/EOF cancellation"]
fn dropping_desktop_owner_cancels_provider_connection() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let (_dir, path) = config(&server);
    let (tx, rx) = mpsc::channel();
    let http = std::thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        body(&mut stream);
        tx.send(()).unwrap();
        let mut byte = [0];
        match stream.read(&mut byte) {
            Ok(0) => (),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::BrokenPipe
                        | std::io::ErrorKind::UnexpectedEof
                ) => {}
            other => panic!("provider connection was not cancelled: {other:?}"),
        }
    });
    let controller = controller(path);
    controller.start("fixture".into()).unwrap();
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(controller);
    http.join().unwrap();
}

#[test]
#[ignore = "explicit Windows immediate-start cancellation"]
fn stop_during_startup_cannot_be_overwritten_by_late_start() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let (_dir, path) = config(&server);
    let controller = controller(path);
    controller.start("fixture".into()).unwrap();
    controller.stop();
    let result = wait(&controller, |v| !v.busy);
    assert_eq!(result.kind, Kind::Cancelled as i32);
    assert!(result.played_samples.is_none());
}
