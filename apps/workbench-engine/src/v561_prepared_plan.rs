//! Atomic, session-private collection of complete prepared assets.
use crate::v561_prepared_asset::{Asset, Policy, hex};
use nai_contracts::workbench::WorkbenchError as Error;
use nai_runtime_core::foundation::Operation;
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// Never forward opaque adapter/OS error text into diagnostic artifacts.
pub(crate) fn safe_failure_reason(error: &dyn std::error::Error) -> &'static str {
    Error::from_str_name(&error.to_string())
        .map(|error| error.as_str_name())
        .unwrap_or("PREPARATION_ERROR_UNKNOWN")
}

#[cfg(test)]
mod failure_tests {
    use super::*;

    #[test]
    fn preserves_only_canonical_error_codes() {
        for code in [
            Error::DeadlineExceeded,
            Error::Cancelled,
            Error::ResourceExhausted,
        ] {
            let error: Box<dyn std::error::Error> = code.as_str_name().into();
            assert_eq!(safe_failure_reason(error.as_ref()), code.as_str_name());
        }
    }

    #[test]
    fn rejects_raw_provider_text_and_embedded_codes() {
        for text in [
            "secret=private; body=answer",
            "WORKBENCH_ERROR_CANCELLED private",
            "",
        ] {
            let error: Box<dyn std::error::Error> = text.into();
            assert_eq!(
                safe_failure_reason(error.as_ref()),
                "PREPARATION_ERROR_UNKNOWN"
            );
        }
    }
}

pub struct Plan {
    input: [u8; 32],
    assets: Vec<Arc<Asset>>,
    bytes: u64,
    samples: u64,
    prefetch_max_bytes: u64,
    standby: Arc<crate::v561_standby::Image>,
}
impl Plan {
    pub fn new(
        text: &str,
        assets: Vec<Arc<Asset>>,
        policy: Policy,
        op: &Operation,
    ) -> Result<Self, Error> {
        op.check()?;
        policy.validate()?;
        if text.trim().is_empty() || assets.is_empty() || assets.len() > policy.max_entries {
            return Err(Error::InvalidArgument);
        }
        let mut bytes = 0u64;
        let mut samples = 0u64;
        for asset in &assets {
            op.check()?;
            if !asset.valid() {
                return Err(Error::PermissionDenied);
            }
            bytes = bytes
                .checked_add(asset.bytes())
                .ok_or(Error::ResourceExhausted)?;
            samples = samples
                .checked_add(asset.samples())
                .ok_or(Error::ResourceExhausted)?;
            if asset.bytes() > policy.max_bytes
                || bytes > policy.max_total_bytes
                || samples > 480000
            {
                return Err(Error::ResourceExhausted);
            }
        }
        let mut standby = assets[0].standby_image(op)?;
        standby.restrict_until(
            assets
                .iter()
                .map(|a| a.expires_at())
                .min()
                .ok_or(Error::InvalidArgument)?,
        );
        Ok(Self {
            input: Sha256::digest(text.as_bytes()).into(),
            assets,
            bytes,
            samples,
            prefetch_max_bytes: policy.prefetch_max_bytes,
            standby: Arc::new(standby),
        })
    }
    pub fn valid(&self) -> bool {
        self.assets.iter().all(|asset| asset.valid())
    }
    pub fn matches(&self, text: &str, keys: &[[u8; 32]]) -> bool {
        let input: [u8; 32] = Sha256::digest(text.as_bytes()).into();
        input == self.input
            && keys.len() == self.assets.len()
            && self
                .assets
                .iter()
                .zip(keys)
                .all(|(asset, key)| asset.matches(key))
    }
    pub fn assets(&self) -> &[Arc<Asset>] {
        &self.assets
    }
    pub fn identity(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(self.input);
        for asset in &self.assets {
            hash.update(asset.identity_key());
        }
        hash.finalize().into()
    }
    pub(crate) fn standby_view(
        &self,
        generation: Arc<std::sync::atomic::AtomicU64>,
        expected: u64,
        alive: Arc<std::sync::atomic::AtomicBool>,
    ) -> crate::v561_standby::View {
        crate::v561_standby::View::bind(self.standby.clone(), generation, expected, alive)
    }
    pub(crate) fn standby_metadata(&self) -> serde_json::Value {
        self.standby.metadata()
    }
    pub fn manifest(&self) -> serde_json::Value {
        serde_json::json!({"version":1,"scope":"private_session_prepared_plan",
            "input_sha256":hex(&self.input),"asset_identities":self.assets.iter().map(|asset| asset.manifest()["identity_sha256"].clone()).collect::<Vec<_>>(),
            "total_bytes":self.bytes,"total_samples":self.samples,"model_api_calls":self.assets.len()})
    }
}

#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn play(
    plan: Arc<Plan>,
    keys: &[[u8; 32]],
    session: &mut crate::v561_stream_output::PreparedSession,
    audio: &std::path::Path,
    dir: &std::path::Path,
    prefill: u64,
    operation: Operation,
    output: Option<&mut minifb::Window>,
    standby: Option<&crate::v561_standby::View>,
    cursor: &mut crate::v561_question::Cursor,
    start_sentence: usize,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    play_inner(
        plan,
        keys,
        session,
        audio,
        dir,
        prefill,
        operation,
        output,
        standby,
        cursor,
        start_sentence,
        None,
        1,
        false,
    )
}

#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn play_observed(
    plan: Arc<Plan>,
    keys: &[[u8; 32]],
    session: &mut crate::v561_stream_output::PreparedSession,
    audio: &std::path::Path,
    dir: &std::path::Path,
    prefill: u64,
    operation: Operation,
    output: Option<&mut minifb::Window>,
    standby: Option<&crate::v561_standby::View>,
    cursor: &mut crate::v561_question::Cursor,
    start_sentence: usize,
    encoder: Option<(crate::v561_obs_encoder::Encoder, u32, bool)>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let Some((mut encoder, repetitions, extended_session)) = encoder else {
        return play(
            plan,
            keys,
            session,
            audio,
            dir,
            prefill,
            operation,
            output,
            standby,
            cursor,
            start_sentence,
        );
    };
    let mut report = play_inner(
        plan,
        keys,
        session,
        audio,
        dir,
        prefill,
        operation,
        output,
        standby,
        cursor,
        start_sentence,
        Some(&mut encoder),
        repetitions,
        extended_session,
    )?;
    if encoder.finish_with_tail().is_err() {
        report["obs_failed"] = true.into();
    }
    Ok(report)
}

#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
pub fn play_with_obs(
    plan: Arc<Plan>,
    keys: &[[u8; 32]],
    session: &mut crate::v561_stream_output::PreparedSession,
    audio: &std::path::Path,
    dir: &std::path::Path,
    prefill: u64,
    operation: Operation,
    mut encoder: crate::v561_obs_encoder::Encoder,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut cursor =
        crate::v561_question::Cursor::new(plan.identity().to_vec(), 1, plan.assets.len() as u32)
            .map_err(|_| "OBS prepared cursor rejected")?;
    let standby = plan.standby_view(
        Arc::new(std::sync::atomic::AtomicU64::new(1)),
        1,
        session.liveness().ok_or("OBS standby worker unavailable")?,
    );
    let report = play_inner(
        plan,
        keys,
        session,
        audio,
        dir,
        prefill,
        operation,
        None,
        Some(&standby),
        &mut cursor,
        0,
        Some(&mut encoder),
        1,
        false,
    )?;
    encoder.finish_with_tail()?;
    Ok(report)
}

#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
fn play_inner(
    plan: Arc<Plan>,
    keys: &[[u8; 32]],
    session: &mut crate::v561_stream_output::PreparedSession,
    audio: &std::path::Path,
    dir: &std::path::Path,
    prefill: u64,
    operation: Operation,
    output: Option<&mut minifb::Window>,
    standby: Option<&crate::v561_standby::View>,
    cursor: &mut crate::v561_question::Cursor,
    start_sentence: usize,
    mut encoder: Option<&mut crate::v561_obs_encoder::Encoder>,
    repetitions: u32,
    extended_session: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    operation.check().map_err(|e| e.as_str_name())?;
    if !plan.valid()
        || start_sentence >= plan.assets.len()
        || !cursor.matches(&plan.identity(), plan.assets.len() as u32)
        || cursor.position() != start_sentence as u32
        || keys.len() != plan.assets.len()
        || !plan
            .assets
            .iter()
            .zip(keys)
            .all(|(asset, key)| asset.matches(key))
    {
        return Err("CONTENT_NOT_PREPARED".into());
    }
    if repetitions == 0 || (repetitions > 1 && start_sentence != 0) {
        return Err("OBS_REPEAT_LIMIT".into());
    }
    let plan_len = plan.assets.len();
    let (max_segments, max_samples) = if extended_session {
        (
            nai_media_core::continuous_audio::MAX_SCRIPT_SEGMENTS,
            nai_media_core::continuous_audio::MAX_SCRIPT_SAMPLES,
        )
    } else {
        (
            crate::v561_continuous_session::MAX_SEGMENTS,
            crate::v561_continuous_session::MAX_SAMPLES,
        )
    };
    let end = plan_len
        .checked_mul(repetitions as usize)
        .filter(|end| (*end - start_sentence) <= max_segments as usize)
        .ok_or("OBS_REPEAT_LIMIT")?;
    let samples = (start_sentence..end)
        .map(|index| plan.assets[index % plan_len].samples())
        .sum::<u64>();
    if samples > max_samples {
        return Err("OBS_REPEAT_LIMIT".into());
    }
    std::fs::create_dir_all(dir)?;
    let origin = std::time::Instant::now();
    let mut owned;
    let window = match output {
        Some(window) => window,
        None => {
            owned = session.script_window()?;
            &mut owned
        }
    };
    let mut device = crate::v561_continuous_session::Session::start_bounded(
        audio,
        operation.clone(),
        (end - start_sentence) as u32,
        samples,
    )
    .map_err(|e| e.as_str_name())?;
    let handle = device.handle();
    // A recorder lease starts when pre-roll is sent. Arm it only after the
    // device is ready so device startup cannot consume the first lease.
    if let Some(encoder) = encoder.as_deref_mut() {
        encoder.send_black_silence(crate::v561_obs_encoder::OBS_PREROLL_FRAMES)?;
    }
    let mut reports = Vec::new();
    let mut previous_end = None;
    let mut verified = None;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        for segment in start_sentence..end {
            operation.check().map_err(|e| e.as_str_name())?;
            let index = segment % plan_len;
            if segment != start_sentence && index == 0 {
                if !plan.valid() {
                    return Err("CONTENT_NOT_PREPARED".into());
                }
                cursor
                    .restart_completed()
                    .map_err(|_| "CURSOR_RECEIPT_REJECTED")?;
            }
            let asset = &plan.assets[index];
            let key = keys[index];
            let child = operation
                .model_child(
                    operation
                        .remaining_ms()
                        .map_err(|e| e.as_str_name())?
                        .min(45000),
                )
                .map_err(|e| e.as_str_name())?;
            let next = (segment + 1 < end)
                .then(|| &plan.assets[(segment + 1) % plan_len])
                .filter(|a| a.bytes() <= plan.prefetch_max_bytes)
                .cloned();
            let ahead_op = operation.clone();
            let limit = plan.prefetch_max_bytes;
            let used_snapshot = verified.is_some();
            let observer = encoder.as_ref().map(|_| session.observe_output());
            let obs_standby = if segment > start_sentence {
                standby.map(|view| (view, &operation))
            } else {
                None
            };
            let (outcome, ahead, obs) = std::thread::scope(|scope| {
                let obs = observer
                    .zip(encoder.as_deref_mut())
                    .map(|(observer, encoder)| {
                        scope.spawn(move || {
                            crate::v561_obs_encoder::forward_segment_with_standby(
                                observer,
                                encoder,
                                obs_standby,
                            )
                        })
                    });
                let task = next.map(|asset| {
                    scope.spawn(move || {
                        asset
                            .verify_ahead(limit, &ahead_op)
                            .map(|snapshot| (snapshot, origin.elapsed().as_millis()))
                    })
                });
                let outcome = session.play_prepared_in(
                    asset.clone(),
                    key,
                    audio,
                    &dir.join(format!("sentence-{}", segment + 1)),
                    prefill,
                    child,
                    origin,
                    verified.take(),
                    crate::v561_stream_output::ScriptOutput {
                        window: &mut *window,
                        audio: &handle,
                        standby,
                    },
                );
                if outcome.is_err() {
                    operation.cancel();
                }
                let ahead = task.map(|task| {
                    task.join()
                        .map_err(|_| Error::OutcomeUnknown)
                        .and_then(|r| r)
                });
                (outcome, ahead, obs.map(|thread| thread.join()))
            });
            let mut report = outcome?;
            if let Some(result) = obs {
                let (frames, completed) = result.map_err(|_| "OBS forwarder panicked")??;
                if !completed || frames == 0 {
                    return Err("OBS segment incomplete".into());
                }
                report["obs_forwarded_frames"] = frames.into();
            }
            let ready = ahead.transpose().map_err(|e| e.as_str_name())?;
            report["next_verification_completed_script_ms"] =
                serde_json::json!(ready.as_ref().map(|(_, at)| *at));
            verified = ready.map(|(snapshot, _)| snapshot);
            report["verified_snapshot_used"] = used_snapshot.into();
            let start = report["device_first_played_script_ms"]
                .as_u64()
                .ok_or("missing device start")?;
            report["gap_from_previous_device_end_ms"] =
                serde_json::json!(previous_end.map(|end| start.saturating_sub(end)));
            previous_end = Some(
                report["device_completed_script_ms"]
                    .as_u64()
                    .ok_or("missing device end")?,
            );
            let complete = report["status"] == "completed"
                && report["played_samples"].as_u64() == Some(asset.samples());
            report["sentence_index"] = (index as u32).into();
            report["repetition_index"] = (segment / plan_len).into();
            if complete {
                cursor
                    .complete(
                        index as u32,
                        asset.samples(),
                        report["played_samples"].as_u64().ok_or("missing receipt")?,
                        true,
                    )
                    .map_err(|_| "CURSOR_RECEIPT_REJECTED")?;
            }
            reports.push(report);
            if !complete {
                return Err("PREPARED_PLAN_STOPPED".into());
            }
        }
        device.finish().map_err(|e| e.as_str_name())?;
        operation.check().map_err(|e| e.as_str_name())?;
        Ok(())
    })();
    let report = serde_json::json!({"scope":"private_session_prepared_plan_playback",
        "status":if result.is_ok(){"completed"}else{"failed"}, "model_api_calls":0,
        "output_window_count":1,"output_device_count":1,"device_pid":device.pid,
        "samples":samples,
        "repetitions":repetitions,"audio_session_startup":device.startup,
        "start_sentence":start_sentence,"next_sentence":cursor.position(),
        "epoch":reports.last().and_then(|r|r["epoch"].as_u64()).unwrap_or(0),
        "played_samples":reports.iter().filter_map(|r|r["played_samples"].as_u64()).sum::<u64>(),
        "sentences":reports,"elapsed_ms":origin.elapsed().as_millis()});
    serde_json::to_writer_pretty(std::fs::File::create_new(dir.join("script.json"))?, &report)?;
    result?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standby_cannot_outlive_shortest_asset_or_resume_revoked_view() {
        use std::sync::atomic::{AtomicBool, AtomicU64};
        let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let op = Operation::model_task(1000).unwrap();
        let plan = Plan::new(
            "a",
            vec![
                Arc::new(crate::v561_prepared_asset::tests::asset()),
                Arc::new(crate::v561_prepared_asset::tests::with_expiry(until)),
            ],
            policy(),
            &op,
        )
        .unwrap();
        assert_eq!(plan.standby.expires_at(), until);
        let view = plan.standby_view(
            Arc::new(AtomicU64::new(1)),
            1,
            Arc::new(AtomicBool::new(true)),
        );
        assert!(view.pixels().is_some());
        view.revoke();
        assert!(view.pixels().is_none());
    }
    fn policy() -> Policy {
        Policy {
            max_bytes: 4194304,
            ttl_ms: 60000,
            max_entries: 4,
            max_total_bytes: 16777216,
            prefetch_max_bytes: 4194304,
        }
    }
    #[test]
    fn resume_identity_binds_whole_text_order_and_cursor_count() {
        let op = Operation::model_task(1000).unwrap();
        let asset = Arc::new(crate::v561_prepared_asset::tests::asset());
        let assets = vec![asset.clone(), asset];
        let plan = Plan::new("原稿", assets.clone(), policy(), &op).unwrap();
        let changed = Plan::new("改稿", assets, policy(), &op).unwrap();
        assert_ne!(plan.identity(), changed.identity());
        let mut cursor = crate::v561_question::Cursor::new(plan.identity().to_vec(), 7, 2).unwrap();
        cursor.complete(0, 24000, 24000, true).unwrap();
        assert_eq!(cursor.position(), 1);
        assert!(cursor.matches(&plan.identity(), 2));
        assert!(!cursor.matches(&changed.identity(), 2));
        assert!(!cursor.matches(&plan.identity(), 3));
    }
    #[test]
    fn whole_text_and_every_leaf_identity_must_match() {
        let assets = (0..4)
            .map(|_| Arc::new(crate::v561_prepared_asset::tests::asset()))
            .collect();
        let plan = Plan::new(
            "四句完整正文",
            assets,
            policy(),
            &Operation::model_task(1000).unwrap(),
        )
        .unwrap();
        assert!(plan.matches("四句完整正文", &[[9; 32]; 4]));
        assert!(!plan.matches("四句完整正文 ", &[[9; 32]; 4]));
        assert!(!plan.matches("四句完整正文", &[[9; 32]; 3]));
        assert!(!plan.matches("四句完整正文", &[[8; 32]; 4]));
        assert_eq!(plan.manifest()["total_samples"], 8000);
    }
    #[test]
    fn publication_rejects_empty_over_count_total_budget_and_cancel() {
        let op = Operation::model_task(1000).unwrap();
        assert!(Plan::new("x", vec![], policy(), &op).is_err());
        let assets: Vec<_> = (0..6)
            .map(|_| Arc::new(crate::v561_prepared_asset::tests::asset()))
            .collect();
        assert!(Plan::new("x", assets.clone(), policy(), &op).is_err());
        let mut p = policy();
        p.max_entries = 8;
        p.max_total_bytes = 4194304;
        assert!(matches!(
            Plan::new("x", assets.clone(), p, &op),
            Err(Error::ResourceExhausted)
        ));
        op.cancel();
        assert!(Plan::new("x", assets, policy(), &op).is_err());
    }
}
