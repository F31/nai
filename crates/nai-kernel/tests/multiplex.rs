//! Pro multi-session isolation: private namespaces, a read-only share whitelist
//! and per-family admission (A11).
use nai_contracts::v1::ErrorCode;
use nai_kernel::multiplex::{
    Admission, AdmissionFamily, IsolationViolation, LANES, Lane, MAX_SESSIONS, Multiplexer,
    Request, SHAREABLE, SessionNamespace, SharedKind, SharedPool, audit_isolation,
};

fn two_sessions() -> Multiplexer {
    let mut sessions = Multiplexer::new();
    sessions.open("a", 1).unwrap();
    sessions.open("b", 1).unwrap();
    sessions
}

// --- per-session namespace ---------------------------------------------------

#[test]
fn two_sessions_get_independent_epochs() {
    let mut sessions = two_sessions();
    assert_eq!(sessions.advance_epoch("a").unwrap(), 2);
    assert_eq!(sessions.namespace("a").unwrap().epoch, 2);
    assert_eq!(
        sessions.namespace("b").unwrap().epoch,
        1,
        "one session's restart must not move another"
    );
}

#[test]
fn a_lane_is_reachable_only_by_its_owner() {
    let mut sessions = two_sessions();
    sessions.bind("a", Lane::Task).unwrap();
    let grant = sessions.access("a", "a", Lane::Task).unwrap();
    assert_eq!(grant.owner, "a");
    assert_eq!(grant.epoch, 1);
    assert_eq!(grant.lane, Lane::Task);
    let refusal = sessions.access("b", "a", Lane::Task).unwrap_err();
    assert_eq!(refusal.code, ErrorCode::Conflict);
    assert_eq!(refusal.reason, "lane belongs to another session");
}

#[test]
fn an_unbound_lane_is_absent_for_its_own_session() {
    let sessions = two_sessions();
    let error = sessions.access("a", "a", Lane::Output).unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityAbsent);
    assert_eq!(error.reason, "lane is not bound");
}

#[test]
fn a_lane_on_an_unknown_session_is_absent() {
    let sessions = two_sessions();
    let error = sessions.access("c", "c", Lane::Task).unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityAbsent);
}

#[test]
fn each_lane_binds_once_per_session() {
    let mut sessions = two_sessions();
    sessions.bind("a", Lane::Audio).unwrap();
    assert_eq!(
        sessions.bind("a", Lane::Audio).unwrap_err().code,
        ErrorCode::Conflict
    );
    // The same lane on another session is not a conflict: lanes are per session.
    sessions.bind("b", Lane::Audio).unwrap();
    assert_eq!(sessions.namespace("b").unwrap().lanes.len(), 1);
}

#[test]
fn all_six_lanes_are_addressable() {
    let mut sessions = two_sessions();
    for lane in LANES {
        sessions.bind("a", *lane).unwrap();
    }
    assert_eq!(sessions.namespace("a").unwrap().lanes.len(), LANES.len());
}

#[test]
fn opening_the_same_session_twice_is_a_conflict() {
    let mut sessions = two_sessions();
    assert_eq!(sessions.open("a", 2).unwrap_err().code, ErrorCode::Conflict);
}

#[test]
fn the_session_registry_is_bounded() {
    let mut sessions = Multiplexer::new();
    for index in 0..MAX_SESSIONS {
        sessions.open(&format!("s{index}"), 1).unwrap();
    }
    assert_eq!(
        sessions.open("one-too-many", 1).unwrap_err().code,
        ErrorCode::ResourceExhausted
    );
}

#[test]
fn closing_a_session_retires_its_namespace() {
    let mut sessions = two_sessions();
    sessions.close("a").unwrap();
    assert!(sessions.namespace("a").is_none());
    assert_eq!(
        sessions.access("a", "a", Lane::Task).unwrap_err().code,
        ErrorCode::CapabilityAbsent
    );
    assert_eq!(
        sessions.close("a").unwrap_err().code,
        ErrorCode::CapabilityAbsent
    );
    // The other session is untouched by the close.
    assert!(sessions.namespace("b").is_some());
}

// --- share whitelist ---------------------------------------------------------

#[test]
fn the_shareable_set_is_exactly_three_kinds() {
    assert_eq!(SHAREABLE.len(), 3);
    for kind in SHAREABLE {
        assert!(!kind.as_str().is_empty());
    }
}

#[test]
fn a_shared_pool_admits_up_to_its_holder_limit() {
    let mut pool = SharedPool::new(2).unwrap();
    pool.acquire("a", SharedKind::ConnectionPool, "pool/main")
        .unwrap();
    pool.acquire("b", SharedKind::ConnectionPool, "pool/main")
        .unwrap();
    assert_eq!(pool.holders(SharedKind::ConnectionPool, "pool/main"), 2);
    assert_eq!(
        pool.acquire("c", SharedKind::ConnectionPool, "pool/main")
            .unwrap_err()
            .code,
        ErrorCode::ResourceExhausted
    );
}

#[test]
fn reacquiring_is_idempotent_for_the_same_session() {
    let mut pool = SharedPool::new(1).unwrap();
    pool.acquire("a", SharedKind::ImmutableModel, "model/avatar")
        .unwrap();
    pool.acquire("a", SharedKind::ImmutableModel, "model/avatar")
        .unwrap();
    assert_eq!(pool.holders(SharedKind::ImmutableModel, "model/avatar"), 1);
}

#[test]
fn shared_state_has_no_write_path() {
    let pool = SharedPool::new(4).unwrap();
    let error = pool
        .write("a", SharedKind::KnowledgeSnapshot, "space/main")
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::PermissionDenied);
    assert_eq!(error.reason, "shared resource is read-only");
}

#[test]
fn releasing_a_lease_frees_a_holder_slot() {
    let mut pool = SharedPool::new(2).unwrap();
    pool.acquire("a", SharedKind::KnowledgeSnapshot, "space/main")
        .unwrap();
    pool.acquire("b", SharedKind::KnowledgeSnapshot, "space/main")
        .unwrap();
    pool.release("a", SharedKind::KnowledgeSnapshot, "space/main")
        .unwrap();
    assert_eq!(pool.holders(SharedKind::KnowledgeSnapshot, "space/main"), 1);
    pool.acquire("c", SharedKind::KnowledgeSnapshot, "space/main")
        .unwrap();
    assert_eq!(
        pool.release("a", SharedKind::KnowledgeSnapshot, "space/main")
            .unwrap_err()
            .code,
        ErrorCode::CapabilityAbsent
    );
}

#[test]
fn a_zero_holder_limit_is_refused() {
    assert_eq!(
        SharedPool::new(0).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
}

// --- capacity admission ------------------------------------------------------

fn limits() -> Vec<(AdmissionFamily, u64)> {
    vec![
        (AdmissionFamily::GpuThroughput, 100),
        (AdmissionFamily::Vram, 10),
        (AdmissionFamily::AudioTiming, 4),
        (AdmissionFamily::ApiQuota, 50),
    ]
}

#[test]
fn families_are_admitted_separately() {
    let mut admission = Admission::new(&limits());
    // Plenty of GPU headroom, but video memory is nearly full.
    admission
        .admit("a", &[Request::new(AdmissionFamily::GpuThroughput, 10)])
        .unwrap();
    admission
        .admit("a", &[Request::new(AdmissionFamily::Vram, 9)])
        .unwrap();
    let error = admission
        .admit("b", &[Request::new(AdmissionFamily::Vram, 2)])
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceExhausted);
    assert_eq!(error.reason, "family capacity exceeded");
}

#[test]
fn a_refusal_leaves_a_live_session_untouched() {
    let mut admission = Admission::new(&limits());
    admission
        .admit("a", &[Request::new(AdmissionFamily::GpuThroughput, 80)])
        .unwrap();
    let before_total = admission.used_total(AdmissionFamily::GpuThroughput);
    let before_a = admission.used_by("a", AdmissionFamily::GpuThroughput);

    // b asks for more than the remaining headroom, so nothing is committed.
    assert!(
        admission
            .admit("b", &[Request::new(AdmissionFamily::GpuThroughput, 30)])
            .is_err()
    );
    assert_eq!(admission.used_by("b", AdmissionFamily::GpuThroughput), 0);
    assert_eq!(
        admission.used_by("a", AdmissionFamily::GpuThroughput),
        before_a
    );
    assert_eq!(
        admission.used_total(AdmissionFamily::GpuThroughput),
        before_total,
        "the on-air session's numbers must not move on a refusal"
    );
}

#[test]
fn a_partial_breach_commits_nothing_at_all() {
    let mut admission = Admission::new(&limits());
    // The GPU request fits; the audio-timing request does not. Two-phase
    // admission means the fitting family is not committed either.
    let requests = [
        Request::new(AdmissionFamily::GpuThroughput, 10),
        Request::new(AdmissionFamily::AudioTiming, 99),
    ];
    assert!(admission.admit("a", &requests).is_err());
    assert_eq!(admission.used_total(AdmissionFamily::GpuThroughput), 0);
    assert_eq!(admission.used_total(AdmissionFamily::AudioTiming), 0);
}

#[test]
fn multiple_requests_for_one_family_are_summed_before_the_check() {
    let mut admission = Admission::new(&limits());
    let requests = [
        Request::new(AdmissionFamily::Vram, 3),
        Request::new(AdmissionFamily::Vram, 4),
    ];
    // 3 + 4 = 7 fits under 10.
    admission.admit("a", &requests).unwrap();
    assert_eq!(admission.used_by("a", AdmissionFamily::Vram), 7);
    // Now 7 + 4 = 11 does not, even though each half looks small on its own.
    assert!(
        admission
            .admit("a", &[Request::new(AdmissionFamily::Vram, 4)])
            .is_err()
    );
}

#[test]
fn an_unstated_family_admits_nothing() {
    let mut admission = Admission::new(&[(AdmissionFamily::GpuThroughput, 100)]);
    assert_eq!(admission.limit(AdmissionFamily::ApiQuota), 0);
    assert_eq!(
        admission
            .admit("a", &[Request::new(AdmissionFamily::ApiQuota, 1)])
            .unwrap_err()
            .code,
        ErrorCode::ResourceExhausted
    );
}

#[test]
fn a_zero_amount_request_is_refused() {
    let mut admission = Admission::new(&limits());
    assert_eq!(
        admission
            .admit("a", &[Request::new(AdmissionFamily::GpuThroughput, 0)])
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
}

#[test]
fn release_frees_only_that_session() {
    let mut admission = Admission::new(&limits());
    admission
        .admit("a", &[Request::new(AdmissionFamily::Vram, 4)])
        .unwrap();
    admission
        .admit("b", &[Request::new(AdmissionFamily::Vram, 4)])
        .unwrap();
    admission.release("a");
    assert_eq!(admission.used_by("a", AdmissionFamily::Vram), 0);
    assert_eq!(admission.used_by("b", AdmissionFamily::Vram), 4);
    // The freed headroom is reusable: 4 + 6 = 10, exactly the limit.
    admission
        .admit("c", &[Request::new(AdmissionFamily::Vram, 6)])
        .unwrap();
}

// --- audit -------------------------------------------------------------------

#[test]
fn a_healthy_registry_audits_clean() {
    let mut sessions = two_sessions();
    sessions.bind("a", Lane::Task).unwrap();
    sessions.bind("b", Lane::Audio).unwrap();
    let mut pool = SharedPool::new(4).unwrap();
    pool.acquire("a", SharedKind::ImmutableModel, "model/avatar")
        .unwrap();
    let audit = audit_isolation(&sessions.sessions(), &pool);
    assert!(audit.is_clean(), "unexpected: {:?}", audit.violations);
}

#[test]
fn audit_reports_a_duplicate_session_id() {
    let duplicate = vec![
        SessionNamespace {
            session_id: "a".into(),
            epoch: 1,
            lanes: Default::default(),
        },
        SessionNamespace {
            session_id: "a".into(),
            epoch: 2,
            lanes: Default::default(),
        },
    ];
    let pool = SharedPool::new(4).unwrap();
    let audit = audit_isolation(&duplicate, &pool);
    assert_eq!(
        audit.violations,
        vec![IsolationViolation {
            token: "two namespaces share a session id",
            subject: "a".into(),
        }]
    );
}
