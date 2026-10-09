use nai_contracts::v1::{MediaState, SessionLifecycle as Life, SessionMode};
use nai_kernel::duty::{DUTY_REVOKE_DELAY, DutyEnforcement};
use nai_kernel::session::Session;
use std::time::{Duration, Instant};

fn running() -> Session {
    let mut session = Session::new("s", "w").unwrap();
    for state in [Life::Preparing, Life::Ready, Life::Running] {
        session.transition(true, state, true).unwrap();
    }
    session
}

#[test]
fn mute_takeover_and_stop_clear_duty_and_do_not_restore_auto_on_confirmation() {
    use nai_contracts::v1::{CommandAction, ControlCommand, RequestContext};
    for action in [
        CommandAction::Mute,
        CommandAction::Takeover,
        CommandAction::Stop,
    ] {
        let now = Instant::now();
        let mut session = running();
        session
            .confirm_duty(true, "operator", Duration::from_secs(30), now)
            .unwrap();
        session
            .change_mode(true, SessionMode::AutoSupervised, true, now)
            .unwrap();
        let snapshot = session.snapshot();
        let command = ControlCommand {
            command_id: format!("{action:?}"),
            action: action as i32,
            ttl_ms: 1000,
            expected_state_revision: snapshot.state_revision,
            context: Some(RequestContext {
                workspace_id: "w".into(),
                session_id: "s".into(),
                generation_epoch: Some(snapshot.generation_epoch),
                remaining_budget_ms: 1000,
                ..Default::default()
            }),
            ..Default::default()
        };
        session
            .command(&command, true, true, now, now, now + Duration::from_secs(5))
            .unwrap();
        assert!(!session.duty_valid(now));
        assert!(!session.auto_reply_permitted(now));
        assert_eq!(session.snapshot().mode, SessionMode::Assisted as i32);
        let after = session.snapshot().clone();
        session
            .command(&command, true, true, now, now, now + Duration::from_secs(5))
            .unwrap();
        assert_eq!(session.snapshot(), &after, "retry must not revoke twice");
        session
            .confirm_duty(true, "operator", Duration::from_secs(30), now)
            .unwrap();
        assert!(!session.auto_reply_permitted(now));
        if action != CommandAction::Stop {
            session
                .change_mode(true, SessionMode::AutoSupervised, true, now)
                .unwrap();
            assert!(session.auto_reply_permitted(now));
        }
    }
}

#[test]
fn duty_grant_expires_and_heartbeat_never_renews() {
    let now = Instant::now();
    let ttl = Duration::from_secs(10);
    let mut session = running();
    assert!(session.confirm_duty(true, "  ", ttl, now).is_err());
    assert!(
        session
            .confirm_duty(true, "op", Duration::ZERO, now)
            .is_err()
    );
    assert!(session.confirm_duty(false, "op", ttl, now).is_err());
    assert!(!session.duty_valid(now));
    session.confirm_duty(true, "op", ttl, now).unwrap();
    assert!(session.duty_valid(now));
    assert!(session.duty_valid(now + ttl - Duration::from_millis(1)));
    for index in 0..1000 {
        session.note_heartbeat(now + ttl / 2 + Duration::from_millis(index));
    }
    assert!(
        session.duty_valid(now + ttl - Duration::from_millis(1)),
        "heartbeats must not extend the duty lease"
    );
    assert!(
        !session.duty_valid(now + ttl),
        "expiry boundary is exclusive"
    );
}

#[test]
fn auto_reply_needs_running_supervised_mode_and_valid_duty() {
    let now = Instant::now();
    let mut session = running();
    assert!(
        session
            .change_mode(true, SessionMode::AutoSupervised, true, now)
            .is_err()
    );
    assert_eq!(session.snapshot().mode, SessionMode::Assisted as i32);
    session
        .confirm_duty(true, "op", Duration::from_secs(5), now)
        .unwrap();
    session
        .change_mode(true, SessionMode::AutoSupervised, true, now)
        .unwrap();
    assert!(session.auto_reply_permitted(now));
    assert!(
        !session.auto_reply_permitted(now + Duration::from_secs(5)),
        "expiry stops new automatic replies immediately"
    );
}

#[test]
fn duty_expiry_revokes_output_after_bounded_delay_and_only_confirmation_restores() {
    let now = Instant::now();
    let ttl = Duration::from_secs(2);
    let mut session = running();
    session.confirm_duty(true, "op", ttl, now).unwrap();
    session
        .change_mode(true, SessionMode::AutoSupervised, true, now)
        .unwrap();
    let epoch_before = session.snapshot().generation_epoch;
    let expiry = now + ttl;
    assert_eq!(
        session.enforce_duty(expiry),
        DutyEnforcement::HaltedPendingRevoke
    );
    assert_eq!(session.snapshot().generation_epoch, epoch_before);
    assert!(!session.auto_reply_permitted(expiry));
    // A reconnect-like liveness signal must not restore automatic authority.
    session.note_heartbeat(expiry + Duration::from_millis(100));
    assert!(!session.auto_reply_permitted(expiry + Duration::from_millis(100)));
    assert_eq!(
        session.enforce_duty(expiry + DUTY_REVOKE_DELAY - Duration::from_millis(1)),
        DutyEnforcement::HaltedPendingRevoke
    );
    assert_eq!(
        session.enforce_duty(expiry + DUTY_REVOKE_DELAY),
        DutyEnforcement::Revoked
    );
    assert!(session.snapshot().generation_epoch > epoch_before);
    assert_eq!(
        session.snapshot().media_state,
        MediaState::Muted as i32,
        "revocation mutes output"
    );
    assert_eq!(
        session.enforce_duty(expiry + Duration::from_secs(5)),
        DutyEnforcement::Revoked,
        "enforcement is idempotent"
    );
    assert!(!session.auto_reply_permitted(expiry + Duration::from_secs(5)));
    session
        .confirm_duty(true, "op", ttl, expiry + Duration::from_secs(5))
        .unwrap();
    assert!(session.auto_reply_permitted(expiry + Duration::from_secs(5)));
}

#[test]
fn sessions_without_a_lease_have_nothing_to_enforce() {
    let now = Instant::now();
    let mut session = running();
    assert_eq!(session.enforce_duty(now), DutyEnforcement::Valid);
    assert!(!session.auto_reply_permitted(now));
}
