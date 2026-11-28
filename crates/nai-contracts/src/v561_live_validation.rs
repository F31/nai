use crate::v561_live::{Command, Event, Kind, Request};
pub fn resume_intent(value: &crate::v561_live::ResumeIntent) -> bool {
    value.plan_sha256.len() == 32
        && value.content_generation > 0
        && (1..=32).contains(&value.sentence_count)
        && value.sentence_index < value.sentence_count
}
pub fn question_task(value: &crate::v561_live::QuestionTask) -> bool {
    value.task_id > 0
        && value.generation > 0
        && bounded_text(&value.question)
        && (1..=45000).contains(&value.budget_ms)
        && value.resume.as_ref().is_none_or(resume_intent)
}
pub fn question_result(value: &crate::v561_live::QuestionResult) -> bool {
    value.task_id > 0 && value.generation > 0 && bounded_text(&value.answer)
}
fn bounded_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 4096 && value.chars().count() <= 1024
}
pub fn request(value: &Request, last: u64) -> bool {
    value.version == 1
        && last.checked_add(1) == Some(value.sequence)
        && (value.reply.is_none() || value.command == Command::ReplyText as i32)
        && match Command::try_from(value.command) {
            Ok(Command::ReplyText) => {
                value.text.is_empty()
                    && value
                        .reply
                        .as_ref()
                        .is_some_and(|r| bounded_text(&r.question) && bounded_text(&r.answer))
                    && prost::Message::encoded_len(value) <= 8192
            }
            Ok(
                Command::Start
                | Command::PrepareContent
                | Command::PlayPrepared
                | Command::PlayPreparedObs
                | Command::Ask,
            ) => {
                !value.text.trim().is_empty()
                    && value.text.chars().count() <= 1024
                    && value.text.len() <= 4096
            }
            Ok(Command::Prepare | Command::Stop | Command::Shutdown) => value.text.is_empty(),
            _ => false,
        }
}
pub fn event(value: &Event, last: u64) -> bool {
    value.version == 1
        && last.checked_add(1) == Some(value.sequence)
        && value.code.len() <= 64
        && value
            .obs_port
            .is_none_or(|port| (1024..=65535).contains(&port))
        && value
            .code
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        && value.samples.is_none_or(|s| s <= 32 * 144000)
        && value
            .played_samples
            .is_none_or(|p| value.samples.is_some_and(|s| p <= s))
        && match Kind::try_from(value.kind) {
            Ok(Kind::Preparing | Kind::Starting | Kind::Stopping | Kind::ContentPreparing) => {
                value.busy
            }
            Ok(Kind::ContentPrepared) => {
                !value.busy
                    && value.content_prepared
                    && value.visual_prepared
                    && value.tts_prepared
                    && value.worker_pid > 0
            }
            Ok(Kind::Prepared) => {
                !value.busy && value.visual_prepared && value.tts_prepared && value.worker_pid > 0
            }
            Ok(Kind::Completed) => {
                !value.busy
                    && value
                        .samples
                        .is_some_and(|s| s > 0 && value.played_samples == Some(s))
            }
            Ok(Kind::Unprepared | Kind::Stopped | Kind::Failed) => !value.busy,
            _ => false,
        }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ask_is_question_only_and_cannot_smuggle_answer_or_cursor() {
        let mut r = Request {
            version: 1,
            sequence: 1,
            command: Command::Ask as i32,
            text: "如何使用？".into(),
            ..Default::default()
        };
        assert!(request(&r, 0));
        r.reply = Some(crate::v561_live::ReplyText {
            question: "q".into(),
            answer: "a".into(),
        });
        assert!(!request(&r, 0));
        r.reply = None;
        r.text = " ".into();
        assert!(!request(&r, 0));
        r.text = "字".repeat(1025);
        assert!(!request(&r, 0));
    }
    #[test]
    fn supplied_reply_is_explicit_bounded_and_never_smuggled_on_stop() {
        let mut r = Request {
            version: 1,
            sequence: 1,
            command: Command::ReplyText as i32,
            reply: Some(crate::v561_live::ReplyText {
                question: "如何使用？".into(),
                answer: "先准备内容。".into(),
            }),
            ..Default::default()
        };
        assert!(request(&r, 0));
        r.command = Command::Stop as i32;
        assert!(!request(&r, 0));
        r.command = Command::ReplyText as i32;
        r.text = "hidden".into();
        assert!(!request(&r, 0));
        r.text.clear();
        r.reply.as_mut().unwrap().answer.clear();
        assert!(!request(&r, 0));
        r.reply = None;
        assert!(!request(&r, 0));
    }
    #[test]
    fn question_tasks_results_and_cursor_are_bounded_and_roundtrip() {
        use crate::v561_live::{QuestionResult, QuestionTask, ResumeIntent};
        use prost::Message;
        let intent = ResumeIntent {
            plan_sha256: vec![7; 32],
            content_generation: 2,
            sentence_index: 1,
            sentence_count: 3,
        };
        let mut task = QuestionTask {
            task_id: 7,
            generation: 4,
            question: "产品有什么功能？".into(),
            resume: Some(intent),
            budget_ms: 45000,
        };
        assert!(question_task(&task));
        assert_eq!(
            QuestionTask::decode(task.encode_to_vec().as_slice()).unwrap(),
            task
        );
        task.resume.as_mut().unwrap().sentence_index = 3;
        assert!(!question_task(&task));
        task.resume.as_mut().unwrap().sentence_index = 1;
        task.resume.as_mut().unwrap().plan_sha256.pop();
        assert!(!question_task(&task));
        task.resume = None;
        for budget in [0, 45001] {
            task.budget_ms = budget;
            assert!(!question_task(&task));
        }
        task.budget_ms = 45000;
        task.question = "字".repeat(1025);
        assert!(!question_task(&task));
        let mut result = QuestionResult {
            task_id: 7,
            generation: 4,
            answer: "支持语音讲解。".into(),
        };
        assert!(question_result(&result));
        assert_eq!(
            QuestionResult::decode(result.encode_to_vec().as_slice()).unwrap(),
            result
        );
        result.answer = " ".into();
        assert!(!question_result(&result));
    }
    #[test]
    fn request_replay_unknown_and_text_on_stop_rejected() {
        let mut r = Request {
            version: 1,
            sequence: 1,
            command: Command::Stop as i32,
            text: String::new(),
            reply: None,
        };
        assert!(request(&r, 0));
        assert!(!request(&r, 1));
        r.text = "hidden".into();
        assert!(!request(&r, 0));
        r.command = 99;
        assert!(!request(&r, 0));
        r.command = Command::Start as i32;
        assert!(request(&r, 0));
        r.text = " ".into();
        assert!(!request(&r, 0));
        assert!(!request(&r, u64::MAX));
        r.text = "准备内容。".into();
        for command in [
            Command::PrepareContent,
            Command::PlayPrepared,
            Command::PlayPreparedObs,
        ] {
            r.command = command as i32;
            assert!(request(&r, 0));
            r.text.clear();
            assert!(!request(&r, 0));
            r.text = "准备内容。".into();
        }
    }
    #[test]
    fn completion_and_prepared_are_distinct_facts() {
        let mut e = Event {
            version: 1,
            sequence: 1,
            kind: Kind::Prepared as i32,
            visual_prepared: true,
            tts_prepared: true,
            worker_pid: 1,
            ..Default::default()
        };
        assert!(event(&e, 0));
        e.tts_prepared = false;
        assert!(!event(&e, 0));
        e.tts_prepared = true;
        e.visual_prepared = false;
        assert!(!event(&e, 0));
        e.kind = Kind::Completed as i32;
        assert!(!event(&e, 0));
        e.samples = Some(1);
        e.played_samples = Some(1);
        assert!(event(&e, 0));
        e.played_samples = Some(2);
        assert!(!event(&e, 0));
        e.code = "provider body!".into();
        assert!(!event(&e, 0));
        e.code.clear();
        e.kind = Kind::ContentPrepared as i32;
        e.samples = None;
        e.played_samples = None;
        e.visual_prepared = true;
        e.tts_prepared = true;
        e.worker_pid = 1;
        e.content_prepared = false;
        assert!(!event(&e, 0));
        e.content_prepared = true;
        assert!(event(&e, 0));
    }
    #[test]
    fn script_totals_are_bounded_separately_from_sentence_gpu_admission() {
        let mut e = Event {
            version: 1,
            sequence: 1,
            kind: Kind::Completed as i32,
            samples: Some(288000),
            played_samples: Some(288000),
            ..Default::default()
        };
        assert!(event(&e, 0));
        e.samples = Some(32 * 144000 + 1);
        e.played_samples = e.samples;
        assert!(!event(&e, 0));
    }
}
