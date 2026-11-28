//! Silent bounded preparation only. No device, media cache or playback permission.
use nai_contracts::workbench::WorkbenchError as Error;
use nai_llm_openai::cosyvoice::{self, Config, Summary};
use nai_runtime_core::foundation::Operation;
use std::time::Instant;

pub fn validate_text(text: &str) -> Result<(), Error> {
    if text.trim().is_empty() || text.chars().count() > 64 {
        return Err(Error::InvalidArgument);
    }
    Ok(())
}

pub fn prepare(
    config: &Config,
    text: &str,
    parent: &Operation,
    attempts: &mut u32,
) -> Result<Summary, Error> {
    validate_text(text)?;
    let operation = parent.model_child(config.deadline_ms)?;
    // A previous cancelled delivery may still be draining on the single server.
    // Waiting does not issue another synthesis or reset the child deadline.
    cosyvoice::wait_idle(config, operation.clone())?;
    discard(config, text, operation, |config, text, op, accept| {
        *attempts += 1;
        cosyvoice::stream(config, text, op, accept)
    })
}

fn discard(
    config: &Config,
    text: &str,
    operation: Operation,
    synth: impl FnOnce(
        &Config,
        &str,
        Operation,
        &mut dyn FnMut(&[u8]) -> Result<(), Error>,
    ) -> Result<Summary, Error>,
) -> Result<Summary, Error> {
    validate_text(text)?;
    operation.check()?;
    let mut bounded = config.clone();
    bounded.max_samples = bounded.max_samples.min(144000);
    if bounded.max_samples == 0 {
        return Err(Error::InvalidArgument);
    }
    let mut samples = 0u64;
    let began = Instant::now();
    let mut first = None;
    let summary = synth(&bounded, text, operation.clone(), &mut |bytes| {
        operation.check()?;
        if bytes.is_empty()
            || !bytes.len().is_multiple_of(2)
            || bytes.len() as u64 / 2 > bounded.max_samples - samples
        {
            return Err(Error::ProtocolError);
        }
        samples += bytes.len() as u64 / 2;
        first.get_or_insert(began.elapsed().as_millis());
        Ok(()) // Drop this borrowed chunk; never open a device or retain PCM.
    })?;
    operation.check()?;
    if first.is_none() || samples == 0 || summary.samples != samples {
        return Err(Error::ProtocolError);
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Config {
        Config {
            endpoint: "http://127.0.0.1:50000/inference_zero_shot".into(),
            reference_file: "E:/test/reference.wav".into(),
            prompt_text: "<|endofprompt|>参考".into(),
            deadline_ms: 1000,
            max_samples: 480000,
            lookahead: Default::default(),
        }
    }
    fn summary(samples: u64) -> Summary {
        Summary {
            samples,
            first_pcm_ms: 1,
            complete_ms: 2,
            transport_eof_ms: 2,
        }
    }
    #[test]
    fn silence_discards_all_chunks_and_requires_complete_sample_count() {
        let result = discard(
            &config(),
            "语音准备。",
            Operation::model_task(1000).unwrap(),
            |cfg, _, _, push| {
                assert_eq!(cfg.max_samples, 144000);
                push(&[1, 0, 2, 0])?;
                push(&[3, 0])?;
                Ok(summary(3))
            },
        )
        .unwrap();
        assert_eq!(result.samples, 3);
        let result = discard(
            &config(),
            "准备。",
            Operation::model_task(1000).unwrap(),
            |_, _, _, push| {
                push(&[1, 0])?;
                Ok(summary(2))
            },
        );
        assert!(matches!(result, Err(Error::ProtocolError)));
    }
    #[test]
    fn cancellation_during_and_after_delivery_cannot_become_prepared() {
        for during in [true, false] {
            let op = Operation::model_task(1000).unwrap();
            let cancel = op.clone();
            let result = discard(&config(), "准备。", op, |_, _, _, push| {
                push(&[1, 0])?;
                cancel.cancel();
                if during {
                    push(&[2, 0])?;
                }
                Ok(summary(1))
            });
            assert!(matches!(result, Err(Error::Cancelled)));
        }
    }
    #[test]
    fn invalid_text_or_cancelled_parent_never_calls_synthesis() {
        for text in [" ".to_string(), "字".repeat(65)] {
            assert!(
                discard(
                    &config(),
                    &text,
                    Operation::model_task(1000).unwrap(),
                    |_, _, _, _| panic!("must not call")
                )
                .is_err()
            );
        }
        let op = Operation::model_task(1000).unwrap();
        op.cancel();
        assert!(
            discard(&config(), "准备。", op, |_, _, _, _| panic!(
                "must not call"
            ))
            .is_err()
        );
    }
    #[test]
    fn malformed_or_over_budget_pcm_and_provider_failure_are_rejected() {
        for bytes in [vec![], vec![1], vec![0; 288002]] {
            assert!(
                discard(
                    &config(),
                    "准备。",
                    Operation::model_task(1000).unwrap(),
                    |_, _, _, push| {
                        push(&bytes)?;
                        Ok(summary(1))
                    }
                )
                .is_err()
            );
        }
        assert!(matches!(
            discard(
                &config(),
                "准备。",
                Operation::model_task(1000).unwrap(),
                |_, _, _, _| Err(Error::CapabilityUnavailable)
            ),
            Err(Error::CapabilityUnavailable)
        ));
    }
}
