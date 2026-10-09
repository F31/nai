use nai_contracts::v1::*;
use nai_kernel::{
    CallContext,
    platform::PlatformLink,
    scheduler::{EventIngress, Ingest},
};
use std::{collections::BTreeMap, time::Duration};

fn context() -> CallContext {
    CallContext::new(Duration::from_secs(5)).unwrap()
}
fn capabilities(context: &CallContext, revision: &str) -> PlatformCapabilities {
    PlatformCapabilities {
        platform: "test-platform".into(),
        app_scope: "test-app".into(),
        room_id: "test-room".into(),
        connected: true,
        permissions: Some(PermissionSnapshot {
            revision: revision.into(),
            authority_ref: "test-authority".into(),
            valid_until_utc_ms: Some(context.now_utc_ms() + 30000),
            capabilities: BTreeMap::from([
                (
                    "platform.events@1/COMMENT".into(),
                    PermissionState::Granted as i32,
                ),
                (
                    "platform.events@1/GIFT".into(),
                    PermissionState::Granted as i32,
                ),
            ]),
        }),
    }
}
fn link() -> PlatformLink {
    PlatformLink::new("session", 0, "test-platform", "test-app").unwrap()
}
fn event(context: &CallContext, generation: u64) -> PlatformEvent {
    let now = context.now_utc_ms();
    PlatformEvent {
        event_id: "test-event".into(),
        session_id: "session".into(),
        platform: "test-platform".into(),
        app_scope: "test-app".into(),
        room_id: "test-room".into(),
        room_source_ref: "test-verified-room".into(),
        source: EventSource::SourceOfficial as i32,
        kind: EventKind::Comment as i32,
        permission_authority_ref: "test-authority".into(),
        payload_version: 1,
        received_at_utc_ms: now,
        event_at_utc_ms: Some(now),
        expires_at_utc_ms: now + 10000,
        generation_epoch: Some(0),
        connection_generation: generation,
        ..Default::default()
    }
}

#[test]
fn scoped_admission_and_failure_do_not_replace_live_connection() {
    let context = context();
    let mut link = link();
    let caps = capabilities(&context, "r1");
    assert!(
        link.admit(&caps, "test-verified-room", false, &context)
            .is_err()
    );
    assert!(link.validate(&event(&context, 1), &context).is_err());
    assert_eq!(
        link.admit(&caps, "test-verified-room", true, &context)
            .unwrap(),
        1
    );
    assert!(
        link.admit(&capabilities(&context, "r2"), "other-room", true, &context)
            .is_err()
    );
    link.validate(&event(&context, 1), &context).unwrap();
}

#[test]
fn disconnect_reconnect_revokes_old_events_and_requires_new_permissions() {
    let context = context();
    let mut link = link();
    let caps = capabilities(&context, "r1");
    link.admit(&caps, "test-verified-room", true, &context)
        .unwrap();
    let old = event(&context, 1);
    link.disconnect();
    link.disconnect();
    assert!(link.validate(&old, &context).is_err());
    assert!(
        link.admit(&caps, "test-verified-room", true, &context)
            .is_err()
    );
    assert_eq!(
        link.admit(
            &capabilities(&context, "r2"),
            "test-verified-room",
            true,
            &context
        )
        .unwrap(),
        2
    );
    assert!(link.validate(&old, &context).is_err());
    link.validate(&event(&context, 2), &context).unwrap();
}

#[test]
fn rejects_missing_epoch_wrong_scope_and_nonofficial_sources() {
    let context = context();
    let mut link = link();
    link.admit(
        &capabilities(&context, "r1"),
        "test-verified-room",
        true,
        &context,
    )
    .unwrap();
    let base = event(&context, 1);
    let mutations: [fn(&mut PlatformEvent); 12] = [
        |e| e.generation_epoch = None,
        |e| e.generation_epoch = Some(1),
        |e| e.connection_generation = 0,
        |e| e.session_id = "other".into(),
        |e| e.platform = "other".into(),
        |e| e.app_scope = "other".into(),
        |e| e.room_id = "old-room".into(),
        |e| e.room_source_ref = "unverified".into(),
        |e| e.permission_authority_ref = "other".into(),
        |e| e.source = EventSource::SourceManual as i32,
        |e| e.source = EventSource::SourceReplay as i32,
        |e| e.source = 999,
    ];
    for mutate in mutations {
        let mut e = base.clone();
        mutate(&mut e);
        assert!(link.validate(&e, &context).is_err());
    }
}

#[test]
fn missing_denied_unknown_permissions_and_types_are_not_granted() {
    for permission in [
        PermissionState::Unknown as i32,
        PermissionState::Denied as i32,
        999,
    ] {
        let context = context();
        let mut link = link();
        let mut caps = capabilities(&context, "r1");
        caps.permissions
            .as_mut()
            .unwrap()
            .capabilities
            .insert("platform.events@1/COMMENT".into(), permission);
        link.admit(&caps, "test-verified-room", true, &context)
            .unwrap();
        assert!(link.validate(&event(&context, 1), &context).is_err());
    }
    let context = context();
    let mut link = link();
    link.admit(
        &capabilities(&context, "r1"),
        "test-verified-room",
        true,
        &context,
    )
    .unwrap();
    for kind in [
        EventKind::Follow as i32,
        EventKind::EventUnknown as i32,
        999,
    ] {
        let mut e = event(&context, 1);
        e.kind = kind;
        assert!(link.validate(&e, &context).is_err());
    }
}

#[test]
fn admission_bounds_and_permission_expiry_fail_closed() {
    let context = context();
    for mutate in [0, 1, 2, 3, 4, 5, 6, 7] {
        let mut link = link();
        let mut caps = capabilities(&context, "r1");
        match mutate {
            0 => caps.connected = false,
            1 => caps.room_id.clear(),
            2 => caps.permissions = None,
            3 => caps.permissions.as_mut().unwrap().valid_until_utc_ms = None,
            4 => caps.permissions.as_mut().unwrap().valid_until_utc_ms = Some(context.now_utc_ms()),
            5 => {
                caps.permissions.as_mut().unwrap().valid_until_utc_ms =
                    Some(context.now_utc_ms() + 240000)
            }
            6 => caps.permissions.as_mut().unwrap().authority_ref.clear(),
            _ => caps.room_id = "x".repeat(16385),
        }
        assert!(
            link.admit(&caps, "test-verified-room", true, &context)
                .is_err()
        );
    }
    let mut link = link();
    let mut caps = capabilities(&context, "short");
    caps.permissions.as_mut().unwrap().valid_until_utc_ms = Some(context.now_utc_ms() + 50);
    link.admit(&caps, "test-verified-room", true, &context)
        .unwrap();
    std::thread::sleep(Duration::from_millis(60));
    assert!(link.validate(&event(&context, 1), &context).is_err());
}

#[test]
fn event_time_size_version_and_call_cancellation_are_checked() {
    let context = context();
    let mut link = link();
    link.admit(
        &capabilities(&context, "r1"),
        "test-verified-room",
        true,
        &context,
    )
    .unwrap();
    for mutate in 0..7 {
        let mut e = event(&context, 1);
        match mutate {
            0 => e.event_id.clear(),
            1 => e.payload_version = 2,
            2 => e.text = "x".repeat(65537),
            3 => e.expires_at_utc_ms = context.now_utc_ms(),
            4 => e.expires_at_utc_ms = context.now_utc_ms() + 120000,
            5 => e.received_at_utc_ms += 10000,
            _ => e.event_at_utc_ms = Some(e.received_at_utc_ms + 10000),
        }
        assert!(link.validate(&e, &context).is_err());
    }
    let cancelled = CallContext::new(Duration::from_secs(5)).unwrap();
    cancelled.cancel();
    assert_eq!(
        link.validate(&event(&context, 1), &cancelled)
            .unwrap_err()
            .code,
        ErrorCode::Cancelled as i32
    );
    let expired = CallContext::new(Duration::ZERO).unwrap();
    assert_eq!(
        link.validate(&event(&context, 1), &expired)
            .unwrap_err()
            .code,
        ErrorCode::DeadlineExceeded as i32
    );
}

#[test]
fn queued_events_recheck_connection_and_dedup_does_not_cross_generations() {
    let context = context();
    let mut link = link();
    let mut ingress = EventIngress::new("session", 4, 4, 8).unwrap();
    link.admit(
        &capabilities(&context, "r1"),
        "test-verified-room",
        true,
        &context,
    )
    .unwrap();
    let old = event(&context, 1);
    link.validate(&old, &context).unwrap();
    assert_eq!(
        ingress.push(old.clone(), context.now_utc_ms()).unwrap(),
        Ingest::Queued
    );
    assert_eq!(
        ingress.push(old, context.now_utc_ms()).unwrap(),
        Ingest::Duplicate
    );
    link.disconnect();
    link.admit(
        &capabilities(&context, "r2"),
        "test-verified-room",
        true,
        &context,
    )
    .unwrap();
    let fresh = event(&context, 2);
    link.validate(&fresh, &context).unwrap();
    assert_eq!(
        ingress.push(fresh, context.now_utc_ms()).unwrap(),
        Ingest::Queued
    );
    let old = ingress.pop(context.now_utc_ms()).unwrap();
    assert!(link.validate(&old, &context).is_err());
    let fresh = ingress.pop(context.now_utc_ms()).unwrap();
    link.validate(&fresh, &context).unwrap();
}
