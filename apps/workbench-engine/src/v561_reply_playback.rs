//! Single task admission, answer resolution and verified whole-sentence recovery.
#[cfg(windows)]
use crate::{
    v561_prepared_plan::Plan,
    v561_question::{Authority, Cursor, Slot},
    v561_stream_output::PreparedSession,
};

#[cfg(windows)]
pub struct Context<'a> {
    pub slot: &'a mut Slot,
    pub cursor: &'a mut Cursor,
    pub plan: std::sync::Arc<Plan>,
    pub keys: &'a [[u8; 32]],
    pub content_generation: u64,
    pub generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
    pub expected: u64,
    pub task_id: u64,
    pub question: &'a str,
    pub answer: Option<&'a str>,
    pub llm: Option<&'a crate::v561_reply_llm::Config>,
    pub source: &'a str,
}

#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
pub fn execute(
    ctx: Context<'_>,
    session: &mut PreparedSession,
    tts: nai_llm_openai::cosyvoice::Config,
    audio: &std::path::Path,
    dir: &std::path::Path,
    prefill: u64,
    window: &mut Option<minifb::Window>,
    standby: &mut Option<crate::v561_standby::View>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    use nai_contracts::v561_live::{QuestionResult, QuestionTask};
    use nai_runtime_core::foundation::Operation;
    use std::{
        sync::{Arc, atomic::Ordering},
        time::Instant,
    };
    let began = Instant::now();
    let identity = ctx.plan.identity();
    let worker = session.liveness().ok_or("VISUAL_UNAVAILABLE")?;
    let live_worker = worker.clone();
    let generation = ctx.generation.clone();
    let expected = ctx.expected;
    let live_plan = ctx.plan.clone();
    let op = Operation::model_task(45000)
        .map_err(|e| e.as_str_name())?
        .bind_permission(Arc::new(move || {
            generation.load(Ordering::Acquire) == expected
                && live_worker.load(Ordering::Acquire)
                && live_plan.valid()
        }))
        .map_err(|e| e.as_str_name())?;
    let authority = || Authority {
        generation: ctx.generation.load(Ordering::Acquire),
        worker_alive: worker.load(Ordering::Acquire),
        content_valid: ctx.plan.valid(),
        content_generation: ctx.content_generation,
        plan_sha256: &identity,
        sentence_count: ctx.plan.assets().len() as u32,
    };
    let result = (|| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        if !ctx
            .cursor
            .matches(&identity, ctx.plan.assets().len() as u32)
        {
            return Err("RESUME_IDENTITY_CHANGED".into());
        }
        let resume = ctx.cursor.resume().ok_or("REPLY_NOT_AVAILABLE")?;
        ctx.slot
            .queue(
                QuestionTask {
                    task_id: ctx.task_id,
                    generation: ctx.expected,
                    question: ctx.question.into(),
                    resume: Some(resume),
                    budget_ms: op.remaining_ms().map_err(|e| e.as_str_name())?,
                },
                &authority(),
                Instant::now(),
            )
            .map_err(|_| "REPLY_ADMISSION_FAILED")?;
        ctx.slot
            .generate(ctx.task_id, ctx.expected, &authority(), Instant::now())
            .map_err(|_| "REPLY_CANCELLED")?;
        let view = ctx
            .plan
            .standby_view(ctx.generation.clone(), ctx.expected, worker.clone());
        if window.is_none() {
            *window = Some(session.standby_window(Some(&view))?);
        }
        *standby = Some(view);
        let output = window.as_mut().ok_or("OUTPUT_UNAVAILABLE")?;
        let view = standby.as_ref().ok_or("OUTPUT_UNAVAILABLE")?;
        crate::v561_presentation::require_render_area(output.get_size(), &op)?;
        let window_identity = format!("{:p}", output.get_window_handle());
        let (w, h) = view.dimensions();
        let mut screen = vec![0; w * h];
        let mut waiting_updates = 0u64;
        let generated;
        let supplied = match ctx.answer {
            Some(text) => text,
            None => {
                generated = crate::v561_reply_llm::while_pumping(
                    &op,
                    || {
                        crate::v561_reply_llm::generate(
                            ctx.llm.ok_or("LLM_NOT_CONFIGURED")?,
                            ctx.question,
                            ctx.source,
                            ctx.task_id,
                            &op,
                            dir,
                        )
                        .map_err(|e| e.to_string())
                    },
                    || {
                        crate::v561_standby::fill(&mut screen, Some(view));
                        output
                            .update_with_buffer(&screen, w, h)
                            .map_err(|_| "OUTPUT_UNAVAILABLE".to_owned())?;
                        crate::v561_presentation::require_render_area(output.get_size(), &op)
                            .map_err(str::to_owned)?;
                        waiting_updates += 1;
                        if !output.is_open() || output.is_key_down(minifb::Key::Escape) {
                            return Err("SCRIPT_STOPPED".into());
                        }
                        Ok(())
                    },
                )?;
                &generated
            }
        };
        let text = ctx
            .slot
            .answer(
                QuestionResult {
                    task_id: ctx.task_id,
                    generation: ctx.expected,
                    answer: supplied.into(),
                },
                &authority(),
                Instant::now(),
            )
            .map_err(|_| "REPLY_CANCELLED")?;
        let answer = crate::v561_stream_script::play_with_output(
            session,
            tts,
            &text,
            audio,
            &dir.join("answer"),
            prefill,
            op.model_child(op.remaining_ms().map_err(|e| e.as_str_name())?)
                .map_err(|e| e.as_str_name())?,
            Some(output),
            Some(view),
        )?;
        if answer["status"] != "completed" {
            return Err("REPLY_PLAY_FAILED".into());
        }
        op.check().map_err(|e| e.as_str_name())?;
        let resume = ctx
            .slot
            .answer_completed(ctx.task_id, ctx.expected, &authority(), Instant::now())
            .map_err(|_| "RESUME_REVOKED")?
            .ok_or("RESUME_REVOKED")?;
        let resumed = crate::v561_prepared_plan::play(
            ctx.plan.clone(),
            ctx.keys,
            session,
            audio,
            &dir.join("resume"),
            prefill,
            op.model_child(op.remaining_ms().map_err(|e| e.as_str_name())?)
                .map_err(|e| e.as_str_name())?,
            window.as_mut(),
            standby.as_ref(),
            ctx.cursor,
            resume.sentence_index as usize,
        )?;
        op.check().map_err(|e| e.as_str_name())?;
        ctx.slot
            .resumed(ctx.task_id, ctx.expected, &authority(), Instant::now())
            .map_err(|_| "RESUME_REVOKED")?;
        let samples = answer["samples"]
            .as_u64()
            .ok_or("MISSING_SAMPLES")?
            .checked_add(resumed["samples"].as_u64().ok_or("MISSING_SAMPLES")?)
            .ok_or("SAMPLE_OVERFLOW")?;
        let played = answer["played_samples"]
            .as_u64()
            .ok_or("MISSING_RECEIPT")?
            .checked_add(
                resumed["played_samples"]
                    .as_u64()
                    .ok_or("MISSING_RECEIPT")?,
            )
            .ok_or("SAMPLE_OVERFLOW")?;
        if samples == 0 || samples != played {
            return Err("INCOMPLETE_REPLY_RESUME".into());
        }
        Ok(
            serde_json::json!({"scope":if ctx.answer.is_some(){"supplied_text_reply_and_sentence_resume_not_llm"}else{"llm_reply_and_sentence_resume"},"status":"completed",
            "task_id":ctx.task_id,"resume_sentence":resume.sentence_index,"elapsed_ms":began.elapsed().as_millis(),
            "output_window_identity":window_identity,"llm_waiting_window_updates":waiting_updates,
            "samples":samples,"played_samples":played,"epoch":resumed["epoch"],"worker_pid":session.worker_pid,
            "answer":answer,"resumed":resumed}),
        )
    })();
    if result.is_err() {
        ctx.slot.cancel();
    }
    std::fs::create_dir_all(dir)?;
    serde_json::to_writer_pretty(std::fs::File::create_new(dir.join("reply.json"))?,
        &result.as_ref().cloned().unwrap_or_else(|_|serde_json::json!({"scope":if ctx.answer.is_some(){"supplied_text_reply_and_sentence_resume_not_llm"}else{"llm_reply_and_sentence_resume"},"status":"failed_or_cancelled","resumed":false})))?;
    result
}
