#![cfg(windows)]
#[path = "../src/media_pipe.rs"]
mod media_pipe;
use nai_contracts::workbench::{PreparedMediaDescriptor, WorkbenchError};
#[test]
#[ignore = "requires separately built nai-public-output-keeper; run by media-pipe native verification"]
fn root_cancellation_and_deadline_after_spawn_reap_pipes_and_child() {
    let binary = std::path::PathBuf::from(
        std::env::var_os("NAI_PUBLIC_OUTPUT_KEEPER").expect("explicit fixed child binary"),
    );
    for failure in [WorkbenchError::Cancelled, WorkbenchError::DeadlineExceeded] {
        let file = tempfile::tempfile().unwrap();
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let check = || {
            if calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0 {
                Ok(())
            } else {
                Err(failure)
            }
        };
        assert_eq!(
            media_pipe::verify_at(
                &binary,
                file,
                &PreparedMediaDescriptor {
                    protocol_major: 1,
                    ..Default::default()
                },
                &check
            ),
            Err(failure)
        );
        assert!(calls.load(std::sync::atomic::Ordering::Relaxed) >= 2);
    }
}
