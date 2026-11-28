//! Loopback HTTP transport tests; no model or supplier API is invoked.
use nai_contracts::workbench::{LlmWorkerRequest, WorkbenchError as Error};
use nai_kernel::output_owner::{OutputOwner, Speaker};
use nai_llm_openai::qwen::{self, Config, TtsCache};
use nai_runtime_core::foundation::Operation;
use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

fn bound(root: &Arc<OutputOwner>) -> Operation {
    Operation::model_task(4000)
        .unwrap()
        .bind_permission(Arc::new(root.acquire_callback(Speaker::Narration).unwrap()))
        .unwrap()
}

#[test]
fn owner_loss_cancels_pending_llm_and_tts_without_retry() {
    for tts in [false, true] {
        for action in 0..3 {
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            let url = format!("http://{}/v1", server.server_addr());
            let root = Arc::new(OutputOwner::default());
            let parent = bound(&root);
            let child = parent.model_child(15000).unwrap();
            let (tx, rx) = mpsc::sync_channel(1);
            let worker = std::thread::spawn(move || {
                let result = if tts {
                    let out = qwen::synthesize(
                        &Config {
                            endpoint: url,
                            deadline_ms: 4000,
                            ..Config::default()
                        },
                        "你好",
                        child,
                    );
                    assert!(out.samples.is_empty());
                    (
                        out.request_started,
                        out.error.unwrap().as_str_name().to_string(),
                    )
                } else {
                    let out = nai_llm_openai::run(
                        LlmWorkerRequest {
                            task_id: "owner-test".into(),
                            base_url: url,
                            model: "test".into(),
                            api_key: b"synthetic-key".to_vec(),
                            input: "hello".into(),
                            system: "test".into(),
                            max_output_tokens: 256,
                            deadline_ms: 4000,
                            connection_test: true,
                        },
                        child,
                        true,
                    );
                    assert!(out.text.is_empty());
                    assert!(!out.usage_known);
                    (out.request_started, out.error)
                };
                tx.send(result).unwrap();
            });
            // Hold the HTTP response open so cancellation, rather than a response, wins.
            let held = server
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .unwrap_or_else(|| {
                    panic!(
                        "request not sent: tts={tts} action={action} result={:?}",
                        rx.try_recv()
                    )
                });
            let start = Instant::now();
            match action {
                0 => {
                    root.acquire(Speaker::Human).unwrap();
                }
                1 => root.stop(),
                _ => drop(root),
            }
            let (sent, error) = rx
                .recv_timeout(Duration::from_secs(1))
                .expect("local cancellation bounded");
            assert!(start.elapsed() < Duration::from_secs(1));
            assert!(sent);
            assert_eq!(error, Error::Cancelled.as_str_name());
            worker.join().unwrap();
            drop(held);
            assert!(
                server
                    .recv_timeout(Duration::from_millis(50))
                    .unwrap()
                    .is_none()
            );
        }
    }
}

#[test]
fn child_inherits_owner_and_cannot_rebind_or_read_cached_audio() {
    let root = Arc::new(OutputOwner::default());
    let parent = bound(&root);
    let child = parent.model_child(15000).unwrap();
    assert!(child.remaining_ms().unwrap() <= parent.remaining_ms().unwrap() + 1);
    assert!(child.clone().bind_permission(Arc::new(|| true)).is_err());
    let config = Config::default();
    let mut cache = TtsCache::new(2);
    cache.insert(TtsCache::key(&config, "hello"), vec![1, 2], 24000);
    root.acquire(Speaker::Human).unwrap();
    let out = qwen::synthesize_cached(&mut cache, &config, "hello", child);
    assert_eq!(out.error, Some(Error::Cancelled));
    assert!(!out.request_started);
    assert!(out.samples.is_empty());
    assert!(parent.model_child(1000).is_err());
}
