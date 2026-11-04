use super::*;
use nai_contracts::workbench::{PublicMediaAudio, PublicMediaVisual};
use serde_json::json;
fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012}")
}
fn documents() -> [Value; 4] {
    let reference = |n| json!({"object_id":id(n),"revision":1,"sha256":"a".repeat(64)});
    [
        json!({"schema_version":1,"kind":"show_draft","name":"规格介绍","product":reference(1),"host":reference(2),"series":reference(3),"cues":[{"id":"intro","title":"介绍","talk_track_id":"opening","mandatory_fact_ids":["capacity"],"allow_questions":true}]}),
        serde_json::from_str(include_str!(
            "../../../../tests/fixtures/contracts/preparation.json"
        ))
        .unwrap(),
        json!({"schema_version":1,"kind":"host_draft","name":"助播","style":"friendly","preferences":""}),
        json!({"schema_version":1,"kind":"series_draft","name":"规格","goal":"解释限制"}),
    ]
}
fn object(text: &str) -> PublicGuardObject {
    PublicGuardObject {
        protocol_major: 1,
        policy_id: policy_id().into(),
        workspace_id: id(10),
        session_id: id(11),
        epoch: 1,
        preview_id: id(12),
        preview_sha256: "a".repeat(64),
        task_id: id(13),
        task_sha256: "b".repeat(64),
        snapshot_id: id(14),
        snapshot_sha256: "c".repeat(64),
        facts_sha256: "d".repeat(64),
        caption_sha256: public_caption_hash(text),
        output_target_id: id(20),
        audio: Some(PublicMediaAudio {
            artifact_id: id(15),
            pcm_sha256: "e".repeat(64),
            sample_count: 24000,
            sample_rate: 24000,
            channels: 1,
        }),
        visual: Some(PublicMediaVisual {
            artifact_id: id(16),
            package_sha256: "f".repeat(64),
            license_sha256: "0".repeat(64),
            width: 512,
            height: 288,
            fps: 30,
        }),
    }
}
fn run(d: &[Value; 4], op: &Operation) -> Result<PublicGuardReview> {
    let text = d[1]["talk_tracks"][0]["text"].as_str().unwrap();
    review(
        op,
        object(text),
        &ReviewInput {
            faq_id: None,
            documents: [&d[0], &d[1], &d[2], &d[3]],
            cue_id: "intro",
            text,
            now: 1000,
            source_valid_until: 61000,
            output_id: "00000000-0000-4000-8000-000000000020",
        },
    )
}
#[test]
fn exact_confirmed_physical_specs_pass_with_bounded_non_executable_review() {
    let d = documents();
    let r = run(&d, &Operation::new(4000).unwrap()).unwrap();
    assert!(r.policy_passed);
    assert_eq!(r.reason, Reason::ExactSpecPassed as i32);
    assert_eq!(r.valid_until_ms, 31000);
    assert_eq!(
        r.object_sha256,
        public_guard_hash(r.object.as_ref().unwrap())
    );
    let mut expiry = d;
    expiry[1]["facts"][0]["valid_until_ms"] = 5000.into();
    assert_eq!(
        run(&expiry, &Operation::new(4000).unwrap())
            .unwrap()
            .valid_until_ms,
        5000
    );
}
#[test]
fn rewritten_numeric_money_and_risky_speech_are_not_silently_approved() {
    for n in 0..6 {
        let mut d = documents();
        match n {
            0 => d[1]["talk_tracks"][0]["text"] = "标准款容量大概500毫升。".into(),
            1 => {
                d[1]["facts"][0]["value_type"] = "money".into();
                d[1]["facts"][0]["currency"] = "CNY".into();
            }
            2 => d[1]["talk_tracks"][0]["text"] = "免费领券，永久有效。".into(),
            3 => d[1]["facts"][0]["value"] = "500e100".into(),
            4 => {
                d[1]["name"] = "官方认证水杯".into();
                d[1]["talk_tracks"][0]["text"] =
                    "这是一款官方认证水杯，标准款容量为500毫升。".into();
            }
            _ => {
                d[1]["facts"][0]["conditions"] = "保证改善体质的标准款".into();
                d[1]["talk_tracks"][0]["text"] = "保证改善体质的标准款容量为500毫升。".into();
            }
        }
        match run(&d, &Operation::new(4000).unwrap()) {
            Ok(review) => assert!(!review.policy_passed, "case {n}"),
            Err(e) => {
                assert_eq!(n, 3, "unexpected invalid fixture case {n}");
                assert_eq!(e, Error::InvalidArgument);
            }
        }
    }
}
#[test]
fn literal_prohibitions_and_uninterpretable_rules_fail_closed() {
    for (rule, reason) in [
        ("禁止：500", Reason::ProhibitedPhrase),
        ("不要夸大效果", Reason::UnsupportedRule),
        ("不得宣称治疗效果除非已授权", Reason::UnsupportedRule),
    ] {
        let mut d = documents();
        d[1]["prohibitions"] = json!([rule]);
        assert_eq!(
            run(&d, &Operation::new(4000).unwrap()).unwrap().reason,
            reason as i32
        );
    }
}
#[test]
fn object_hash_covers_resources_scope_and_source_and_invalid_bindings_refuse_review() {
    let d = documents();
    let text = d[1]["talk_tracks"][0]["text"].as_str().unwrap();
    let original = object(text);
    for n in 0..5 {
        let mut changed = original.clone();
        match n {
            0 => changed.epoch += 1,
            1 => changed.audio.as_mut().unwrap().pcm_sha256 = "0".repeat(64),
            2 => changed.visual.as_mut().unwrap().license_sha256 = "1".repeat(64),
            3 => changed.facts_sha256 = "2".repeat(64),
            _ => changed.output_target_id = id(99),
        }
        assert_ne!(public_guard_hash(&original), public_guard_hash(&changed));
    }
    let input = ReviewInput {
        faq_id: None,
        documents: [&d[0], &d[1], &d[2], &d[3]],
        cue_id: "intro",
        text,
        now: 1000,
        source_valid_until: 61000,
        output_id: "00000000-0000-4000-8000-000000000020",
    };
    for n in 0..3 {
        let mut changed = original.clone();
        match n {
            0 => changed.caption_sha256 = "0".repeat(64),
            1 => changed.output_target_id = id(99),
            _ => changed.audio = None,
        };
        assert!(review(&Operation::new(4000).unwrap(), changed, &input).is_err());
    }
}
#[test]
fn cancellation_expiry_and_unconfirmed_facts_never_return_old_pass() {
    let mut d = documents();
    let op = Operation::new(4000).unwrap();
    op.cancel();
    assert_eq!(run(&d, &op), Err(Error::Cancelled));
    d[1]["facts"][0]["valid_until_ms"] = 999.into();
    assert!(run(&d, &Operation::new(4000).unwrap()).is_err());
    d[1]["facts"][0]["valid_until_ms"] = Value::Null;
    d[1]["facts"][0]["authority"] = "unverified".into();
    assert!(run(&d, &Operation::new(4000).unwrap()).is_err());
}
#[test]
fn exact_multi_fact_sentences_are_allowed_but_unbound_or_rewritten_fragments_are_not() {
    let mut d = documents();
    let mut weight = d[1]["facts"][0].clone();
    weight["id"] = "weight".into();
    weight["label"] = "重量".into();
    weight["value"] = "100".into();
    weight["unit"] = "克".into();
    d[1]["facts"].as_array_mut().unwrap().push(weight);
    d[1]["talk_tracks"][0]["fact_ids"] = json!(["capacity", "weight"]);
    d[1]["talk_tracks"][0]["text"] = "标准款容量为500毫升。标准款重量为100克。".into();
    let op = Operation::new(4000).unwrap();
    for (text, want) in [
        ("标准款容量为500毫升。", Reason::ExactSpecPassed),
        ("标准款重量为100克。", Reason::ExactSpecPassed),
        ("容量为500毫升。", Reason::UnsupportedSpeech),
        ("标准款重量为101克。", Reason::UnsupportedSpeech),
    ] {
        let r = policy(
            &op,
            &ReviewInput {
                faq_id: None,
                documents: [&d[0], &d[1], &d[2], &d[3]],
                cue_id: "intro",
                text,
                now: 1000,
                source_valid_until: 61000,
                output_id: "00000000-0000-4000-8000-000000000020",
            },
        )
        .unwrap();
        assert_eq!(r, want);
    }
    d[1]["talk_tracks"][0]["text"] = "标准款容量为500毫升。永久免费。".into();
    assert_eq!(
        policy(
            &op,
            &ReviewInput {
                faq_id: None,
                documents: [&d[0], &d[1], &d[2], &d[3]],
                cue_id: "intro",
                text: "标准款容量为500毫升。",
                now: 1000,
                source_valid_until: 61000,
                output_id: "00000000-0000-4000-8000-000000000020"
            }
        )
        .unwrap(),
        Reason::RiskyContent
    );
}

#[test]
fn selected_faq_must_be_exact_fact_text_not_arbitrary_approved_words() {
    let mut d = documents();
    let text = "标准款容量为500毫升。";
    let check = |d: &[Value; 4], text: &str| {
        review(
            &Operation::new(4000).unwrap(),
            object(text),
            &ReviewInput {
                documents: [&d[0], &d[1], &d[2], &d[3]],
                cue_id: "intro",
                faq_id: Some("capacity_question"),
                text,
                now: 1000,
                source_valid_until: 61000,
                output_id: "00000000-0000-4000-8000-000000000020",
            },
        )
    };
    assert!(check(&d, text).unwrap().policy_passed);
    assert!(!check(&d, "容量是600毫升。").unwrap().policy_passed);
    d[1]["faq"][0]["text"] = "标准款容量为500毫升。保证治愈。".into();
    assert!(
        !check(&d, "标准款容量为500毫升。保证治愈。")
            .unwrap()
            .policy_passed
    );
}

fn software_documents() -> [Value; 4] {
    let mut d = documents();
    let f = &mut d[1]["facts"][0];
    f["label"] = "本机功能".into();
    f["value_type"] = "text".into();
    f["value"] = "手动提词".into();
    f["conditions"] = "本机工作台".into();
    f.as_object_mut().unwrap().remove("unit");
    d[1]["name"] = "nai直播工作台".into();
    d[1]["talk_tracks"][0]["text"] = "本机工作台支持手动提词。".into();
    d[1]["faq"][0]["text"] = "本机工作台支持手动提词。".into();
    d
}
#[test]
fn exact_local_software_features_are_distinct_and_faq_and_sentences_share_policy() {
    for feature in [
        "手动提词",
        "节目编排",
        "资料与FAQ管理",
        "基础二维形象与字幕",
        "真人麦克风接管",
        "问题复盘与下场草稿",
    ] {
        let mut d = software_documents();
        d[1]["facts"][0]["value"] = feature.into();
        d[1]["talk_tracks"][0]["text"] = format!("本机工作台支持{feature}。").into();
        let r = run(&d, &Operation::new(4000).unwrap()).unwrap();
        assert!(r.policy_passed);
        assert_eq!(r.reason, Reason::ExactFeaturePassed as i32);
        assert_eq!(
            r.object.unwrap().policy_id,
            PublicGuardPolicy::ExactSpecsAndSoftwareV5.as_str_name()
        );
    }
    let d = software_documents();
    let text = d[1]["faq"][0]["text"].as_str().unwrap();
    let r = review(
        &Operation::new(4000).unwrap(),
        object(text),
        &ReviewInput {
            documents: [&d[0], &d[1], &d[2], &d[3]],
            cue_id: "intro",
            faq_id: Some("capacity_question"),
            text,
            now: 1000,
            source_valid_until: 61000,
            output_id: &id(20),
        },
    )
    .unwrap();
    assert!(r.policy_passed);
    assert_eq!(r.reason, Reason::ExactFeaturePassed as i32);
    let mut d = software_documents();
    let mut f = d[1]["facts"][0].clone();
    f["id"] = "program".into();
    f["value"] = "节目编排".into();
    d[1]["facts"].as_array_mut().unwrap().push(f);
    d[1]["talk_tracks"][0]["fact_ids"] = json!(["capacity", "program"]);
    d[1]["talk_tracks"][0]["text"] = "本机工作台支持手动提词。本机工作台支持节目编排。".into();
    let text = "本机工作台支持节目编排。";
    assert_eq!(
        policy(
            &Operation::new(4000).unwrap(),
            &ReviewInput {
                documents: [&d[0], &d[1], &d[2], &d[3]],
                cue_id: "intro",
                faq_id: None,
                text,
                now: 1000,
                source_valid_until: 61000,
                output_id: &id(20)
            }
        )
        .unwrap(),
        Reason::ExactFeaturePassed
    );
}
#[test]
fn software_claims_cannot_expand_scope_mix_fact_kinds_or_add_promises() {
    for n in 0..8 {
        let mut d = software_documents();
        match n {
            0 => {
                d[1]["facts"][0]["value"] = "自动获取全部平台弹幕".into();
                d[1]["talk_tracks"][0]["text"] = "本机工作台支持自动获取全部平台弹幕。".into();
            }
            1 => d[1]["facts"][0]["conditions"] = "全部直播平台".into(),
            2 => d[1]["facts"][0]["label"] = "任意声明".into(),
            3 => d[1]["facts"][0]["currency"] = "CNY".into(),
            4 => d[1]["talk_tracks"][0]["text"] = "工作台支持手动提词。".into(),
            5 => {
                d[1]["talk_tracks"][0]["text"] = "本机工作台支持手动提词。也能替代全部主播。".into()
            }
            6 => {
                let f = documents()[1]["facts"][0].clone();
                let mut f = f;
                f["id"] = "physical".into();
                d[1]["facts"].as_array_mut().unwrap().push(f);
                d[1]["talk_tracks"][0]["fact_ids"] = json!(["capacity", "physical"]);
                d[1]["talk_tracks"][0]["text"] =
                    "本机工作台支持手动提词。标准款容量为500毫升。".into();
            }
            _ => d[1]["talk_tracks"][0]["text"] = "本机工作台支持手动提词。保证收益。".into(),
        };
        match run(&d, &Operation::new(4000).unwrap()) {
            Ok(r) => assert!(!r.policy_passed, "case {n}"),
            Err(e) => assert_eq!(e, Error::InvalidArgument, "invalid schema case {n}"),
        }
    }
}
#[test]
fn software_claims_keep_confirmation_expiry_cancel_and_old_policy_rejection() {
    let mut d = software_documents();
    d[1]["facts"][0]["authority"] = "unverified".into();
    assert!(run(&d, &Operation::new(4000).unwrap()).is_err());
    d[1]["facts"][0]["authority"] = "owner_confirmed".into();
    d[1]["facts"][0]["valid_until_ms"] = 999.into();
    assert!(run(&d, &Operation::new(4000).unwrap()).is_err());
    let d = software_documents();
    let op = Operation::new(4000).unwrap();
    op.cancel();
    assert_eq!(run(&d, &op), Err(Error::Cancelled));
    let text = d[1]["talk_tracks"][0]["text"].as_str().unwrap();
    let mut old = object(text);
    let current_hash = public_guard_hash(&old);
    old.policy_id = "nai-public-exact-spec-v4-faq".into();
    assert_ne!(public_guard_hash(&old), current_hash);
    assert_eq!(
        review(
            &Operation::new(4000).unwrap(),
            old,
            &ReviewInput {
                documents: [&d[0], &d[1], &d[2], &d[3]],
                cue_id: "intro",
                faq_id: None,
                text,
                now: 1000,
                source_valid_until: 61000,
                output_id: &id(20)
            }
        ),
        Err(Error::InvalidArgument)
    );
}

#[test]
fn playback_window_uses_source_samples_and_one_lease_margin_without_extending_deadlines() {
    let mut a = PublicMediaAudio {
        artifact_id: id(1),
        pcm_sha256: "a".repeat(64),
        sample_count: 124800,
        sample_rate: 24000,
        channels: 1,
    };
    assert_eq!(required_playback_window_ms(&a).unwrap(), 5700);
    assert!(required_playback_window_ms(&a).unwrap() > 4200);
    a.sample_count = 1;
    assert_eq!(required_playback_window_ms(&a).unwrap(), 501);
    a.sample_count = 480000;
    assert_eq!(required_playback_window_ms(&a).unwrap(), 20500);
    a.sample_count = 0;
    assert_eq!(required_playback_window_ms(&a), Err(Error::InvalidArgument));
    a.sample_count = 480001;
    assert_eq!(required_playback_window_ms(&a), Err(Error::InvalidArgument));
    a.sample_count = 24000;
    a.sample_rate = 0;
    assert_eq!(required_playback_window_ms(&a), Err(Error::InvalidArgument));
}
