use nai_contracts::{candidate_hash, v1::PerformanceCue};

#[test]
fn candidate_hash_is_map_order_independent_and_binds_content_not_approval_ref() {
    let mut first = PerformanceCue {
        speech_text: "original".into(),
        session_id: "s".into(),
        generation_epoch: 7,
        ..Default::default()
    };
    first.fact_versions.insert("b".into(), "2".into());
    first.fact_versions.insert("a".into(), "1".into());
    let mut second = first.clone();
    second.fact_versions.clear();
    second.fact_versions.insert("a".into(), "1".into());
    second.fact_versions.insert("b".into(), "2".into());
    second.approval_ref = "approval".into();
    second.candidate_hash = "self-report".into();
    assert_eq!(candidate_hash(&first), candidate_hash(&second));
    second.speech_text = "altered".into();
    assert_ne!(candidate_hash(&first), candidate_hash(&second));
    second = first.clone();
    second.generation_epoch += 1;
    assert_ne!(candidate_hash(&first), candidate_hash(&second));
}
