use super::*;
#[test]
fn schema_memoization_never_accepts_mutated_content_or_caches_time_authority() {
    let text = json!({"schema_version":1,"kind":"publication_revocation","created_at_ms":100,"created_by":"00000000-0000-4000-8000-000000000001","snapshot":{"object_id":"00000000-0000-4000-8000-000000000002","revision":1,"sha256":"a".repeat(64)},"reason":"memo-valid"})
        .to_string();
    assert!(artifact(&text).is_ok());
    assert!(artifact(&text).is_ok());
    let mut changed: Value = serde_json::from_str(&text).unwrap();
    changed["unexpected"] = true.into();
    assert!(artifact(&changed.to_string()).is_err());
    let op = Operation::new(4000).unwrap();
    let mut d = docs();
    d[1]["facts"][0]["valid_until_ms"] = 150.into();
    assert!(closure(&op, [&d[0], &d[1], &d[2], &d[3]], 100).is_ok());
    assert!(closure(&op, [&d[0], &d[1], &d[2], &d[3]], 150).is_err());
    assert!(VALID_ARTIFACTS.lock().unwrap().len() <= 64);
}
fn docs() -> [Value; 4] {
    let r = json!({"object_id":"00000000-0000-0000-0000-000000000001","revision":1,"sha256":"a".repeat(64)});
    [
        json!({"schema_version":1,"kind":"show_draft","name":"讲解","product":r,"host":r,"series":r,"cues":[{"id":"intro","title":"介绍","talk_track_id":"opening","mandatory_fact_ids":["capacity"],"allow_questions":true}]}),
        serde_json::from_str(include_str!(
            "../../../../tests/fixtures/contracts/preparation.json"
        ))
        .unwrap(),
        json!({"schema_version":1,"kind":"host_draft","name":"主持","style":"concise","preferences":""}),
        json!({"schema_version":1,"kind":"series_draft","name":"系列","goal":"产品说明"}),
    ]
}
#[test]
fn publication_checks_fact_validity_authority_and_exact_mandatory_binding() {
    let op = Operation::new(4000).unwrap();
    let original = docs();
    let run = |d: &[Value; 4]| closure(&op, [&d[0], &d[1], &d[2], &d[3]], 100);
    assert_eq!(
        run(&original).unwrap().dependencies,
        json!([{"cue_id":"intro","fact_ids":["capacity"]}])
    );
    for (field, value) in [
        ("authority", json!("unverified")),
        ("valid_from_ms", json!(101)),
        ("valid_until_ms", json!(100)),
    ] {
        let mut bad = original.clone();
        bad[1]["facts"][0][field] = value;
        assert!(matches!(run(&bad), Err(Error::CapabilityUnavailable)));
    }
    let mut limited = original.clone();
    limited[1]["facts"][0]["valid_until_ms"] = 150.into();
    assert_eq!(run(&limited).unwrap().valid_until_ms, Some(150));
    let mut missing = original.clone();
    missing[0]["cues"][0]["mandatory_fact_ids"] = json!(["missing"]);
    assert!(matches!(run(&missing), Err(Error::InvalidArgument)));
    let mut missing_track = original.clone();
    missing_track[0]["cues"][0]["talk_track_id"] = "absent".into();
    assert!(matches!(run(&missing_track), Err(Error::InvalidArgument)));
    let mut wrong_kind = original.clone();
    wrong_kind[2] = original[3].clone();
    assert!(matches!(run(&wrong_kind), Err(Error::InvalidArgument)));
    op.cancel();
    assert!(matches!(run(&original), Err(Error::Cancelled)));
}
#[test]
fn field_diff_uses_fact_identity_and_never_changes_immutable_inputs() {
    let before = json!({"facts":[{"id":"price","value":"129900"},{"id":"size","value":"500"}]});
    let after = json!({"facts":[{"id":"size","value":"500"},{"id":"price","value":"139900"}]});
    let mut changed = BTreeSet::new();
    diff(&before, &after, "product", &mut changed).unwrap();
    assert_eq!(
        changed,
        BTreeSet::from(["product/facts/price/value".into()])
    );
    assert_eq!(before["facts"][0]["value"], "129900");
}
