use nai_workbench_host::foundation::{audition_nonce, stop_audition};
#[test]
fn priority_stop_requires_no_owner_or_database_and_changes_start_fence() {
    let d = tempfile::tempdir().unwrap();
    let before = audition_nonce(d.path()).unwrap();
    stop_audition(d.path()).unwrap();
    let one = audition_nonce(d.path()).unwrap();
    stop_audition(d.path()).unwrap();
    let two = audition_nonce(d.path()).unwrap();
    assert_ne!(before, one);
    assert_ne!(one, two);
    assert!(
        std::fs::metadata(d.path().join("audition.stop"))
            .unwrap()
            .len()
            <= 64
    );
    assert!(!d.path().join("foundation.db").exists());
    std::fs::write(d.path().join("audition.stop"), vec![0; 65]).unwrap();
    assert!(audition_nonce(d.path()).is_err());
}
#[cfg(unix)]
#[test]
fn priority_stop_rejects_linked_fence() {
    let d = tempfile::tempdir().unwrap();
    let outside = d.path().join("untouched");
    std::fs::write(&outside, b"keep").unwrap();
    std::os::unix::fs::symlink(&outside, d.path().join("audition.stop")).unwrap();
    assert!(stop_audition(d.path()).is_err());
    assert!(audition_nonce(d.path()).is_err());
    assert_eq!(std::fs::read(outside).unwrap(), b"keep");
}
