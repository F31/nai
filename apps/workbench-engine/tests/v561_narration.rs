//! CPU HTTP fixtures: ordered PCM, verified prefetch, priority and cancellation.
use nai_contracts::workbench::WorkbenchError as Error;
use nai_llm_openai::cosyvoice::Config;
use nai_runtime_core::{
    foundation::Operation,
    v561_narration::{Plan, Policy},
};
use nai_workbench_engine::v561_narration::produce;
use sha2::{Digest, Sha256};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

struct Provider {
    config: Config,
    calls: Arc<Mutex<Vec<String>>>,
    receipts: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    _directory: tempfile::TempDir,
}
impl Provider {
    fn new(fail_second: bool) -> Self {
        Self::with_samples(fail_second, 4800, false)
    }
    fn with_samples(fail_second: bool, samples: u64, silent: bool) -> Self {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/inference_zero_shot", server.server_addr());
        let directory = tempfile::tempdir().unwrap();
        let reference = directory.path().join("fixture.wav");
        std::fs::write(&reference, b"reference fixture").unwrap();
        let calls = Arc::new(Mutex::new(Vec::<String>::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let receipts = Arc::new(AtomicU64::new(0));
        let (seen, done, got) = (calls.clone(), stop.clone(), receipts.clone());
        let worker = std::thread::spawn(move || {
            let mut evidence = std::collections::HashMap::new();
            let mut index = 0u64;
            while !done.load(Ordering::Acquire) {
                let Some(mut request) = server.recv_timeout(Duration::from_millis(10)).unwrap()
                else {
                    continue;
                };
                if request.method() == &tiny_http::Method::Post {
                    let mut body = String::new();
                    request
                        .as_reader()
                        .take(32769)
                        .read_to_string(&mut body)
                        .unwrap();
                    let text = body
                        .split("name=\"tts_text\"\r\n\r\n")
                        .nth(1)
                        .unwrap()
                        .split("\r\n--")
                        .next()
                        .unwrap()
                        .to_owned();
                    let value = if text == "第一句。" {
                        1u8
                    } else if text == "第二句。" {
                        2
                    } else if text == "第三句。" {
                        3
                    } else {
                        9
                    };
                    seen.lock().unwrap().push(text.clone());
                    index += 1;
                    let id = format!("{index:032x}");
                    let pcm = vec![if silent { 0 } else { value }; (samples * 2) as usize];
                    let hash = Sha256::digest(&pcm)
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>();
                    evidence.insert(id.clone(),serde_json::json!({"protocol":"nai.cosyvoice.completion.v1","request_id":id,"instance_id":"a".repeat(32),"state":if fail_second && text=="第二句。" {"failed"}else{"completed"},"samples":samples,"sha256":hash}));
                    let mut response = tiny_http::Response::from_data(pcm);
                    for (key, value) in [
                        ("X-Sample-Rate", "24000"),
                        ("X-Audio-Channels", "1"),
                        ("X-Audio-Encoding", "pcm_s16le"),
                        ("X-Model-Stream", "true"),
                        ("X-Nai-Request", id.as_str()),
                        ("X-Nai-Instance", &"a".repeat(32)),
                    ] {
                        response.add_header(tiny_http::Header::from_bytes(key, value).unwrap());
                    }
                    let _ = request.respond(response);
                } else if request.url() == "/health" {
                    let _ = request.respond(tiny_http::Response::from_string(
                        "{\"status\":\"ok\",\"busy\":false}",
                    ));
                } else {
                    let id = request.url().strip_prefix("/nai/tts/status/").unwrap();
                    let data = serde_json::to_vec(evidence.get(id).unwrap()).unwrap();
                    let _ = request.respond(tiny_http::Response::from_data(data));
                    got.fetch_add(1, Ordering::Release);
                }
            }
        });
        Self {
            config: Config {
                endpoint,
                reference_file: reference,
                prompt_text: "fixture<|endofprompt|>".into(),
                deadline_ms: 4000,
                max_samples: 480000,
                lookahead: Policy::default(),
            },
            calls,
            receipts,
            stop,
            worker: Some(worker),
            _directory: directory,
        }
    }
    fn wait_receipts(&self, n: u64) {
        let end = Instant::now() + Duration::from_secs(2);
        while self.receipts.load(Ordering::Acquire) < n {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for Provider {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
use std::io::Read;

#[test]
#[cfg(windows)]
#[ignore = "explicit Windows silent consumer-ready gate test"]
fn prepared_source_waits_for_consumer_ready_and_rejects_lost_gate() {
    use nai_workbench_engine::{v561_stream_audio::play_prepared_with_gate, v561_stream_clock};
    let binary = std::path::PathBuf::from(std::env::var_os("NAI_STREAM_AUDIO_OUTPUT").unwrap());
    let (tx, rx) = v561_stream_clock::channel();
    let (ready, gate) = std::sync::mpsc::sync_channel(1);
    let exe = binary.clone();
    let task = std::thread::spawn(move || {
        play_prepared_with_gate(
            vec![0; 48000],
            &exe,
            Operation::model_task(4000).unwrap(),
            tx,
            11,
            gate,
        )
    });
    for _ in 0..3 {
        let event = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(event.observation().played_sample.unwrap_or(0), 0);
    }
    ready.send(()).unwrap();
    for _ in rx {}
    let report = task.join().unwrap().unwrap();
    assert_eq!(report.played_samples, 24000);
    let (tx, rx) = v561_stream_clock::channel();
    let (ready, gate) = std::sync::mpsc::sync_channel(1);
    drop(ready);
    let consumer = std::thread::spawn(move || for _ in rx {});
    assert!(matches!(
        play_prepared_with_gate(
            vec![0; 48000],
            &binary,
            Operation::model_task(4000).unwrap(),
            tx,
            12,
            gate
        ),
        Err(Error::Cancelled)
    ));
    consumer.join().unwrap();
}

#[test]
#[cfg(windows)]
#[ignore = "explicit Windows prepared silent PCM clock and consumer-loss test"]
fn prepared_pcm_uses_device_clock_and_stops_on_consumer_loss() {
    use nai_workbench_engine::{v561_stream_audio::play_prepared_with_clock, v561_stream_clock};
    let binary = std::path::PathBuf::from(std::env::var_os("NAI_STREAM_AUDIO_OUTPUT").unwrap());
    let (tx, rx) = v561_stream_clock::channel();
    let consumer = std::thread::spawn(move || {
        let mut stopped = false;
        let mut running = 0;
        for event in rx {
            stopped |=
                event.observation().state == nai_contracts::v561_media::ClockState::Stopped as i32;
            running += u32::from(event.running().is_some());
        }
        (stopped, running)
    });
    let report = play_prepared_with_clock(
        vec![0; 48000],
        &binary,
        Operation::model_task(4000).unwrap(),
        tx,
        7,
    )
    .unwrap();
    let (stopped, running) = consumer.join().unwrap();
    assert!(stopped && running > 0);
    assert_eq!(report.played_samples, 24000);
    assert_eq!(report.model_calls, 0);
    assert_eq!(report.epoch, 7);
    let (tx, rx) = v561_stream_clock::channel();
    drop(rx);
    assert!(
        play_prepared_with_clock(
            vec![0; 48000],
            &binary,
            Operation::model_task(4000).unwrap(),
            tx,
            8
        )
        .is_err()
    );
}

#[test]
#[cfg(windows)]
#[ignore = "explicit Windows silent device source sealing and provider failure"]
fn played_pcm_source_requires_device_completion_and_provider_eos() {
    use nai_workbench_engine::{
        v561_pcm_source::Source,
        v561_stream_audio::{Options, audition_plan_with_source},
    };
    let binary = std::path::PathBuf::from(std::env::var_os("NAI_STREAM_AUDIO_OUTPUT").unwrap());
    for failure in [false, true] {
        let provider = Provider::with_samples(failure, 2400, true);
        let mut config = provider.config.clone();
        config.deadline_ms = 15000;
        config.max_samples = 96000;
        let root = Operation::model_task(15000).unwrap();
        let plan = Plan::new("第一句。第二句。", Policy::default(), root.clone()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut source = Source::new(dir.path(), 96000, root.clone()).unwrap();
        let result = audition_plan_with_source(
            config,
            plan,
            &binary,
            Options::default(),
            root,
            None,
            Some(&mut source),
        );
        if failure {
            assert!(result.is_err());
            assert!(source.render().is_err());
        } else {
            let report = result.unwrap();
            let (epoch, render) = source.render().unwrap();
            assert_eq!(epoch, report.epoch);
            assert_eq!(render.samples, report.played_samples);
            assert_eq!(render.samples, 4800);
            assert_eq!(render.pcm_sha256, Sha256::digest(vec![0; 9600]).to_vec());
            assert_eq!(std::fs::read(render.pcm_path).unwrap(), vec![0; 9600]);
        }
    }
}

#[test]
fn one_prefetch_waits_for_device_boundary_and_does_not_start_third() {
    let provider = Provider::new(false);
    let root = Operation::model_task(4000).unwrap();
    let plan = Plan::new("第一句。第二句。第三句。", Policy::default(), root.clone()).unwrap();
    let played = Arc::new(AtomicU64::new(0));
    let clock = played.clone();
    let config = provider.config.clone();
    let values = Arc::new(Mutex::new(Vec::new()));
    let output = values.clone();
    let task = std::thread::spawn(move || {
        produce(config, plan, root, clock, |bytes| {
            record(&output, bytes[0]);
            Ok(())
        })
    });
    provider.wait_receipts(2);
    assert_eq!(*values.lock().unwrap(), [1]);
    assert_eq!(provider.calls.lock().unwrap().len(), 2);
    played.store(4800, Ordering::Release);
    provider.wait_receipts(3);
    assert_eq!(*values.lock().unwrap(), [1, 2]);
    played.store(9600, Ordering::Release);
    wait_values(&values, 3);
    played.store(14400, Ordering::Release);
    let report = task.join().unwrap().unwrap();
    assert_eq!(*values.lock().unwrap(), [1, 2, 3]);
    assert_eq!(report.samples, 14400);
    assert_eq!(report.model_calls, 3);
    assert!(report.overlap_prefetches >= 1);
}
#[test]
fn interjection_discards_old_prefetch_then_resumes_same_narration_sentence() {
    let provider = Provider::new(false);
    let root = Operation::model_task(4000).unwrap();
    let plan = Plan::new("第一句。第二句。", Policy::default(), root.clone()).unwrap();
    let active = plan.clone();
    let played = Arc::new(AtomicU64::new(0));
    let clock = played.clone();
    let config = provider.config.clone();
    let values = Arc::new(Mutex::new(Vec::new()));
    let output = values.clone();
    let task = std::thread::spawn(move || {
        produce(config, active, root, clock, |b| {
            record(&output, b[0]);
            Ok(())
        })
    });
    provider.wait_receipts(2);
    plan.interject("回答。").unwrap();
    provider.wait_receipts(3);
    assert_eq!(*values.lock().unwrap(), [1]);
    played.store(4800, Ordering::Release);
    provider.wait_receipts(4);
    assert_eq!(*values.lock().unwrap(), [1, 9]);
    played.store(9600, Ordering::Release);
    wait_values(&values, 3);
    played.store(14400, Ordering::Release);
    let report = task.join().unwrap().unwrap();
    assert_eq!(*values.lock().unwrap(), [1, 9, 2]);
    assert_eq!(report.model_calls, 4);
    assert_eq!(report.sentences, 3);
}

fn wait_values(values: &Mutex<Vec<u8>>, n: usize) {
    let end = Instant::now() + Duration::from_secs(2);
    while values.lock().unwrap().len() < n {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
#[cfg(windows)]
#[ignore = "explicit Windows independent silent stream with priority IPC"]
fn native_priority_ipc_discards_prefetch_and_resumes_in_one_device_task() {
    use nai_contracts::v561_audition::Kind;
    use nai_workbench_host::v561_audition::Controller;
    let provider = Provider::with_samples(false, 48000, true);
    let path = provider._directory.path().join("voice.json");
    std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"endpoint":provider.config.endpoint,"reference_file":provider.config.reference_file,"prompt_text":provider.config.prompt_text,"deadline_ms":15000,"max_samples":480000})).unwrap()).unwrap();
    let host = Controller::new(
        std::env::var_os("NAI_AUDITION_CONTROL").unwrap().into(),
        std::env::var_os("NAI_STREAM_AUDIO_OUTPUT").unwrap().into(),
        Some(path),
    )
    .unwrap();
    host.start("第一句。第二句。".into()).unwrap();
    let readiness = Instant::now() + Duration::from_secs(6);
    while provider.receipts.load(Ordering::Acquire) < 2 {
        assert!(
            host.status().busy,
            "early native status: {:?}, calls: {:?}",
            host.status(),
            provider.calls.lock().unwrap()
        );
        assert!(
            Instant::now() < readiness,
            "native status: {:?}, calls: {:?}",
            host.status(),
            provider.calls.lock().unwrap()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(host.interject(0, "旧任务不能插播。".into()).is_err());
    host.interject(host.status().task_id, "回答。".into())
        .unwrap();
    provider.wait_receipts(3);
    let end = Instant::now() + Duration::from_secs(10);
    while host.status().busy {
        assert!(Instant::now() < end, "{:?}", host.status());
        std::thread::sleep(Duration::from_millis(10));
    }
    let status = host.status();
    assert_eq!(status.kind, Kind::Completed as i32);
    assert_eq!(status.played_samples, Some(144000));
    assert_eq!(
        *provider.calls.lock().unwrap(),
        ["第一句。", "第二句。", "回答。", "第二句。"]
    );
}
#[test]
fn bad_prefetch_eos_and_parent_stop_never_deliver_next_sentence() {
    for failed in [false, true] {
        let provider = Provider::new(failed);
        let root = Operation::model_task(4000).unwrap();
        let cancel = root.clone();
        let plan = Plan::new("第一句。第二句。", Policy::default(), root.clone()).unwrap();
        let config = provider.config.clone();
        let output = Arc::new(Mutex::new(Vec::new()));
        let values = output.clone();
        let task = std::thread::spawn(move || {
            produce(config, plan, root, Arc::new(AtomicU64::new(0)), |b| {
                record(&values, b[0]);
                Ok(())
            })
        });
        provider.wait_receipts(2);
        if !failed {
            cancel.cancel();
        }
        assert!(matches!(
            task.join().unwrap(),
            Err(Error::Cancelled | Error::ProtocolError)
        ));
        assert_eq!(*output.lock().unwrap(), [1]);
    }
}

fn record(output: &Mutex<Vec<u8>>, value: u8) {
    let mut values = output.lock().unwrap();
    if values.last() != Some(&value) {
        values.push(value);
    }
}

#[test]
#[cfg(windows)]
#[ignore = "explicit Windows stream clock to bounded synthetic frame scheduler; silent PCM"]
fn authenticated_stream_clock_drives_frames_across_sentence_boundary() {
    use nai_contracts::{
        v561_media::{ClockState, FrameRef, PixelFormat},
        v561_media_validation::FrameLimits,
    };
    use nai_media_core::v561_frame_scheduler::Scheduler;
    use nai_workbench_engine::{
        v561_stream_audio::{Options, audition_with_clock},
        v561_stream_clock,
    };
    let provider = Provider::with_samples(false, 48000, true);
    let mut config = provider.config.clone();
    config.deadline_ms = 15000;
    let (tx, rx) = v561_stream_clock::channel();
    let binary = std::path::PathBuf::from(std::env::var_os("NAI_STREAM_AUDIO_OUTPUT").unwrap());
    let task = std::thread::spawn(move || {
        audition_with_clock(
            config,
            "第一句。第二句。".into(),
            &binary,
            Options::default(),
            Some(tx),
        )
    });
    let mut scheduler = None::<Scheduler>;
    let mut frame_sequence = 0u64;
    let mut selected = std::collections::BTreeSet::new();
    let mut stopped = false;
    for event in rx {
        let observation = event.observation();
        if observation.state == ClockState::Unavailable as i32 {
            assert!(event.running().is_none());
            scheduler = Some(
                Scheduler::new(
                    "stream-fixture".into(),
                    observation.epoch,
                    observation.device_generation,
                    FrameLimits {
                        slots: 4,
                        max_width: 2,
                        max_height: 2,
                        max_sample_rate: 24000,
                        max_payload_bytes: 16,
                    },
                    4,
                    4800,
                )
                .unwrap(),
            );
            continue;
        }
        if observation.state == ClockState::Stopped as i32 {
            assert!(event.running().is_none());
            scheduler.as_mut().unwrap().stop();
            stopped = true;
            continue;
        }
        let Some(clock) = event.running() else {
            scheduler.as_mut().unwrap().stop();
            continue;
        };
        let frames = scheduler.as_mut().unwrap();
        while frames.has_capacity() && frame_sequence < 100 {
            let pixels = vec![0; 16];
            let n = frame_sequence;
            frames
                .offer(
                    FrameRef {
                        pool_id: "stream-fixture".into(),
                        epoch: clock.epoch,
                        sequence: Some(n),
                        source_sample: Some(n * 960),
                        source_sample_rate: 24000,
                        pts_ns: Some(n * 40000000),
                        width: 2,
                        height: 2,
                        stride_bytes: 8,
                        pixel_format: PixelFormat::Bgra8 as i32,
                        slot_index: Some((n % 4) as u32),
                        slot_generation: n / 4 + 1,
                        payload_bytes: 16,
                        content_sha256: Sha256::digest(&pixels).to_vec(),
                    },
                    pixels,
                )
                .unwrap();
            frame_sequence += 1;
        }
        if let Some(frame) = frames.present(clock).unwrap() {
            selected.insert(frame.metadata.sequence.unwrap());
        }
    }
    let report = task.join().unwrap().unwrap();
    assert!(stopped);
    assert_eq!(report.played_samples, 96000);
    assert_eq!(report.sentences, 2);
    assert!(report.clock_observations > 2);
    assert!(report.max_clock_age_ms <= 200);
    assert!(selected.iter().any(|n| *n < 50));
    assert!(selected.iter().any(|n| *n >= 50));
}

#[test]
#[cfg(windows)]
#[ignore = "explicit Windows observer disconnect cancellation with silent PCM"]
fn missing_clock_consumer_fails_current_stream_instead_of_blocking_lease() {
    use nai_workbench_engine::{
        v561_stream_audio::{Options, audition_with_clock},
        v561_stream_clock,
    };
    let provider = Provider::with_samples(false, 48000, true);
    let (tx, rx) = v561_stream_clock::channel();
    drop(rx);
    let binary = std::path::PathBuf::from(std::env::var_os("NAI_STREAM_AUDIO_OUTPUT").unwrap());
    let started = Instant::now();
    let result = audition_with_clock(
        provider.config.clone(),
        "第一句。".into(),
        &binary,
        Options::default(),
        Some(tx),
    );
    assert!(matches!(result, Err(Error::Cancelled)));
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[test]
#[cfg(windows)]
#[ignore = "explicit Windows silent short-sentence prefill"]
fn short_sentence_below_prefill_completes_without_waiting_for_final_seal() {
    use nai_workbench_engine::v561_stream_audio::{Options, audition};
    let provider = Provider::with_samples(false, 480, true);
    let binary = std::path::PathBuf::from(std::env::var_os("NAI_STREAM_AUDIO_OUTPUT").unwrap());
    let report = audition(
        provider.config.clone(),
        "第一句。".into(),
        &binary,
        Options::default(),
    )
    .unwrap();
    assert_eq!(report.samples, 480);
    assert_eq!(report.played_samples, 480);
    assert_eq!(report.sentences, 1);
}
