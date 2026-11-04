use nai_contracts::{workbench::*, workbench_wire};
use prost::Message;
use std::path::PathBuf;
use tempfile::tempdir;
fn engine() -> PathBuf {
    PathBuf::from(
        std::env::var_os("NAI_WORKBENCH_ENGINE")
            .expect("build RF01 engine and set NAI_WORKBENCH_ENGINE"),
    )
}
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[test]
fn native_host_uses_real_new_engine_without_key_or_media() {
    let _serial = TEST_LOCK.lock().unwrap();
    let root = tempdir().unwrap();
    for p in [Presentation::ContentOnly, Presentation::Avatar] {
        for prepare in [false, true] {
            let bytes =
                nai_workbench_host::inspect_at(&engine(), root.path(), p as i32, prepare).unwrap();
            let r = WorkbenchResponse::decode(bytes.as_slice()).unwrap();
            assert_eq!(r.handshake.unwrap().protocol_major, workbench_wire::VERSION);
            let workbench_response::Outcome::Capabilities(c) = r.outcome.unwrap() else {
                panic!("expected capabilities")
            };
            assert!(!c.business_ready);
            assert_eq!(c.started_media_processes, 0);
            assert_eq!(c.provider_calls, 0);
            assert_eq!(c.config.unwrap().presentation, p as i32);
        }
    }
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
#[test]
fn invalid_configuration_and_existing_database_do_not_start_business() {
    let _serial = TEST_LOCK.lock().unwrap();
    let root = tempdir().unwrap();
    assert_eq!(
        nai_workbench_host::inspect_at(&engine(), root.path(), 99, false).unwrap_err(),
        "WB_INVALID_ARGUMENT"
    );
    std::fs::write(root.path().join("state.sqlite3"), b"keep-original").unwrap();
    assert!(
        nai_workbench_host::inspect_at(
            &engine(),
            root.path(),
            Presentation::ContentOnly as i32,
            false
        )
        .is_err()
    );
    assert_eq!(
        std::fs::read(root.path().join("state.sqlite3")).unwrap(),
        b"keep-original"
    );
}
