use super::*;
fn authority() -> Authority<'static> {
    Authority {
        generation: 4,
        worker_alive: true,
        content_valid: true,
        content_generation: 2,
        plan_sha256: &[7; 32],
        sentence_count: 3,
    }
}
fn task(id: u64) -> QuestionTask {
    let mut cursor = Cursor::new(vec![7; 32], 2, 3).unwrap();
    cursor.complete(0, 24000, 24000, true).unwrap();
    QuestionTask {
        task_id: id,
        generation: 4,
        question: "支持哪些功能？".into(),
        resume: cursor.resume(),
        budget_ms: 1000,
    }
}
fn result(id: u64) -> QuestionResult {
    QuestionResult {
        task_id: id,
        generation: 4,
        answer: "支持本机语音讲解。".into(),
    }
}

#[test]
fn cursor_requires_complete_samples_and_resumes_interrupted_whole_sentence() {
    let mut c = Cursor::new(vec![7; 32], 2, 3).unwrap();
    for receipt in [
        (0, 24000, 12000, false),
        (1, 24000, 24000, true),
        (0, 0, 0, true),
        (0, 24000, 24001, true),
    ] {
        assert_eq!(
            c.complete(receipt.0, receipt.1, receipt.2, receipt.3),
            Err(Error::Invalid)
        );
    }
    assert_eq!(c.resume().unwrap().sentence_index, 0);
    c.complete(0, 24000, 24000, true).unwrap();
    assert_eq!(c.complete(0, 24000, 24000, true), Err(Error::Invalid));
    // Interrupted second sentence has no complete receipt and must replay whole.
    assert_eq!(c.resume().unwrap().sentence_index, 1);
    c.complete(1, 24000, 24000, true).unwrap();
    c.complete(2, 24000, 24000, true).unwrap();
    assert!(c.resume().is_none());
}
#[test]
fn one_slot_keeps_original_cursor_and_uses_remaining_budget() {
    let now = Instant::now();
    let mut slot = Slot::default();
    let a = authority();
    slot.queue(task(1), &a, now).unwrap();
    assert_eq!(slot.queue(task(2), &a, now), Err(Error::Busy));
    let child = slot
        .generate(1, 4, &a, now + Duration::from_millis(100))
        .unwrap();
    assert_eq!(child.budget_ms, 900);
    assert_eq!(slot.generate(1, 4, &a, now), Err(Error::WrongPhase));
    assert_eq!(
        slot.answer(result(1), &a, now).unwrap(),
        "支持本机语音讲解。"
    );
    assert_eq!(slot.answer(result(1), &a, now), Err(Error::WrongPhase));
    let resume = slot.answer_completed(1, 4, &a, now).unwrap().unwrap();
    assert_eq!(resume.sentence_index, 1);
    assert_eq!(resume.content_generation, 2);
    assert_eq!(slot.phase(), Some(Phase::Resuming));
    slot.resumed(1, 4, &a, now).unwrap();
    assert_eq!(slot.phase(), None);
    assert_eq!(slot.resumed(1, 4, &a, now), Err(Error::Stale));
}
#[test]
fn stop_discards_resume_and_late_result_cannot_clear_new_question() {
    let now = Instant::now();
    let mut slot = Slot::default();
    let a = authority();
    slot.queue(task(1), &a, now).unwrap();
    slot.generate(1, 4, &a, now).unwrap();
    slot.cancel();
    assert_eq!(slot.answer(result(1), &a, now), Err(Error::Stale));
    assert_eq!(slot.queue(task(1), &a, now), Err(Error::Stale));
    slot.queue(task(2), &a, now).unwrap();
    slot.generate(2, 4, &a, now).unwrap();
    assert_eq!(slot.answer(result(1), &a, now), Err(Error::Stale));
    assert_eq!(slot.phase(), Some(Phase::Generating));
    assert!(slot.answer(result(2), &a, now).is_ok());
}
#[test]
fn expired_content_identity_generation_or_worker_revoke_every_phase() {
    let now = Instant::now();
    for phase in [
        Phase::Queued,
        Phase::Generating,
        Phase::Speaking,
        Phase::Resuming,
    ] {
        for change in 0..5 {
            let mut slot = Slot::default();
            let mut a = authority();
            slot.queue(task(1), &a, now).unwrap();
            if phase != Phase::Queued {
                slot.generate(1, 4, &a, now).unwrap();
            }
            if matches!(phase, Phase::Speaking | Phase::Resuming) {
                slot.answer(result(1), &a, now).unwrap();
            }
            if phase == Phase::Resuming {
                slot.answer_completed(1, 4, &a, now).unwrap();
            }
            match change {
                0 => a.content_valid = false,
                1 => a.worker_alive = false,
                2 => a.generation += 1,
                3 => a.content_generation += 1,
                _ => a.plan_sha256 = &[8; 32],
            }
            slot.observe(&a, now);
            assert_eq!(slot.phase(), None);
            assert_eq!(slot.answer_completed(1, 4, &a, now), Err(Error::Stale));
        }
    }
}
#[test]
fn deadline_is_inclusive_and_no_timeout_refresh_or_automatic_retry() {
    let now = Instant::now();
    let a = authority();
    let mut slot = Slot::default();
    slot.queue(task(1), &a, now).unwrap();
    slot.generate(1, 4, &a, now).unwrap();
    assert_eq!(
        slot.answer(result(1), &a, now + Duration::from_millis(1000)),
        Err(Error::Deadline)
    );
    assert_eq!(slot.phase(), None);
    assert_eq!(slot.queue(task(1), &a, now), Err(Error::Stale));
    slot.queue(task(2), &a, now).unwrap();
    slot.observe(&a, now + Duration::from_millis(1000));
    assert_eq!(slot.phase(), None);
}
#[test]
fn malformed_answer_cancels_task_and_answer_without_lecture_has_no_resume() {
    let now = Instant::now();
    let a = authority();
    let mut slot = Slot::default();
    slot.queue(task(1), &a, now).unwrap();
    slot.generate(1, 4, &a, now).unwrap();
    let mut bad = result(1);
    bad.answer = " ".into();
    assert_eq!(slot.answer(bad, &a, now), Err(Error::Invalid));
    assert_eq!(slot.phase(), None);
    let mut next = task(2);
    next.resume = None;
    slot.queue(next, &a, now).unwrap();
    slot.generate(2, 4, &a, now).unwrap();
    slot.answer(result(2), &a, now).unwrap();
    assert_eq!(slot.answer_completed(2, 4, &a, now), Ok(None));
    assert_eq!(slot.phase(), None);
}

#[test]
fn result_transition_rechecks_authority_without_waiting_for_idle_observation() {
    let now = Instant::now();
    let mut a = authority();
    let mut slot = Slot::default();
    slot.queue(task(1), &a, now).unwrap();
    slot.generate(1, 4, &a, now).unwrap();
    a.worker_alive = false;
    assert_eq!(slot.answer(result(1), &a, now), Err(Error::Revoked));
    assert_eq!(slot.phase(), None);
}
#[cfg(windows)]
#[test]
fn only_a_completed_cursor_can_start_another_playback_round() {
    let mut cursor = super::Cursor::new(vec![1; 32], 1, 2).unwrap();
    assert!(cursor.restart_completed().is_err());
    cursor.complete(0, 100, 100, true).unwrap();
    assert!(cursor.restart_completed().is_err());
    cursor.complete(1, 100, 100, true).unwrap();
    cursor.restart_completed().unwrap();
    assert_eq!(cursor.position(), 0);
    assert!(cursor.matches(&[1; 32], 2));
}
