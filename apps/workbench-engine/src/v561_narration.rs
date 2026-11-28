//! Single model request at a time, one bounded verified lookahead buffer.
use nai_contracts::workbench::WorkbenchError as Error;
use nai_llm_openai::cosyvoice::{self, Config};
use nai_runtime_core::{foundation::Operation, v561_narration::Plan};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

pub struct Production {
    pub samples: u64,
    pub transport_eof_at: Instant,
    pub sentences: u32,
    pub overlap_prefetches: u32,
    pub model_calls: u32,
}
pub fn produce(
    config: Config,
    plan: Plan,
    root: Operation,
    played: Arc<AtomicU64>,
    mut accept: impl FnMut(&[u8]) -> Result<(), Error>,
) -> Result<Production, Error> {
    config.lookahead.validate()?;
    let mut total = 0u64;
    let mut sentences = 0u32;
    let mut overlaps = 0u32;
    let mut last_eof = None;
    let mut calls = 0u32;
    loop {
        root.check()?;
        let Some(ticket) = plan.take_for(sentences > 0, &root)? else {
            if played.load(Ordering::Acquire) >= total && plan.close_if_empty()? {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
            continue;
        };
        if sentences as usize >= config.lookahead.max_sentences {
            return Err(Error::ResourceExhausted);
        }
        if calls as usize >= config.lookahead.max_sentences * 2 {
            return Err(Error::ResourceExhausted);
        }
        let mut request = config.clone();
        request.max_samples = request
            .max_samples
            .checked_sub(total)
            .ok_or(Error::ResourceExhausted)?
            .min(request.lookahead.max_sentence_samples);
        if request.max_samples == 0 {
            return Err(Error::ResourceExhausted);
        }
        let prefetch = sentences > 0;
        if prefetch && played.load(Ordering::Acquire) < total {
            overlaps += 1;
        }
        let mut buffer = Vec::new();
        let mut count = 0u64;
        calls += 1;
        let result = cosyvoice::stream(&request, &ticket.text, ticket.operation.clone(), |bytes| {
            plan.check(&ticket)?;
            count = count
                .checked_add((bytes.len() / 2) as u64)
                .ok_or(Error::ResourceExhausted)?;
            if count > request.max_samples {
                return Err(Error::ResourceExhausted);
            }
            if prefetch {
                buffer.extend_from_slice(bytes);
                Ok(())
            } else {
                accept(bytes)
            }
        });
        let summary = match result {
            Err(Error::Cancelled) if root.check().is_ok() => {
                cosyvoice::wait_idle(&config, root.model_child(config.lookahead.idle_wait_ms)?)?;
                continue;
            }
            other => other?,
        };
        let eof_at = Instant::now()
            - Duration::from_millis(
                summary.complete_ms.saturating_sub(summary.transport_eof_ms) as u64
            );
        if summary.samples != count {
            return Err(Error::ProtocolError);
        }
        if prefetch {
            // Current media alone is in the device. No future sentence needs rollback.
            while played.load(Ordering::Acquire) < total {
                root.check()?;
                if plan.check(&ticket).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            if plan.check(&ticket).is_err() {
                cosyvoice::wait_idle(&config, root.model_child(config.lookahead.idle_wait_ms)?)?;
                continue;
            }
        }
        // Commit selects the next current sentence. Later interjections affect only
        // the following preparation, never mutate this sentence halfway through.
        match plan.commit(&ticket) {
            Ok(()) => {}
            Err(Error::Cancelled) if root.check().is_ok() => {
                cosyvoice::wait_idle(&config, root.model_child(config.lookahead.idle_wait_ms)?)?;
                continue;
            }
            Err(e) => return Err(e),
        }
        if prefetch {
            for part in buffer.chunks(9600) {
                root.check()?;
                accept(part)?;
            }
        }
        total = total.checked_add(count).ok_or(Error::ResourceExhausted)?;
        last_eof = Some(eof_at);
        sentences += 1;
    }
    if total == 0 {
        return Err(Error::ProtocolError);
    }
    Ok(Production {
        samples: total,
        transport_eof_at: last_eof.ok_or(Error::ProtocolError)?,
        sentences,
        overlap_prefetches: overlaps,
        model_calls: calls,
    })
}
