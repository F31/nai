//! Human handoff advances the session epoch; an unconfirmed script sentence is never replayed.
use super::*;
use serde_json::{Value, json};
#[allow(clippy::too_many_arguments)]
pub(super) fn control(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    x: &LiveTakeoverControl,
    now: i64,
) -> Result<FoundationResult> {
    let r = x.run.as_ref().ok_or(Error::InvalidArgument)?;
    uuid(&x.takeover_id)?;
    if !x.acknowledge {
        return Err(Error::PermissionDenied);
    }
    let (mut v, rev) = live::load(tx, root, &r.workspace_id, &r.run_id, actor)?;
    let (epoch, stopped) = live::session(tx, &r.workspace_id, &r.run_id, actor)?;
    if epoch != x.expected_epoch
        || (x.resume && (stopped || v["session_epoch"].as_u64() != Some(epoch)))
        || v["sentence_mode"] != true
    {
        return Err(Error::RevisionConflict);
    }
    if x.resume {
        if v["takeover"]["active"] != true || v["takeover"]["takeover_id"] != x.takeover_id {
            return Err(Error::PermissionDenied);
        }
        let p = live::speech_plan(tx, root, fence, op, actor, &r.workspace_id, &r.run_id, now)?
            .live_speech_plan
            .ok_or(Error::IntegrityFailed)?;
        if v["takeover"]["partial_sentence"] == true {
            if !x.skip_partial_sentence {
                return Err(Error::PermissionDenied);
            }
            let index = v["sentence_cursor"]
                .as_u64()
                .ok_or(Error::IntegrityFailed)? as usize;
            let sentence = p.sentences.get(index).ok_or(Error::RevisionConflict)?;
            let last = p
                .sentences
                .get(index + 1)
                .is_none_or(|n| n.cue_id != sentence.cue_id);
            v["sentence_cursor"] = (index + 1).into();
            v["skipped_sentences"] = (v["skipped_sentences"].as_u64().unwrap() + 1).into();
            if last {
                v["cursor"] = (v["cursor"].as_u64().unwrap() + 1).into();
                v["skipped_count"] = (v["skipped_count"].as_u64().unwrap() + 1).into();
                v["current_sentence_index"] = 0.into();
            } else {
                v["current_sentence_index"] =
                    (v["current_sentence_index"].as_u64().unwrap() + 1).into();
            }
        }
        v["takeover"]["active"] = false.into();
        v["takeover"]["resumed_at_ms"] = now.into();
        v["active_approval"] = "".into();
        v["active_bundle"] = "".into();
        v["state"] = if v["sentence_cursor"] == v["sentence_total"] {
            LiveRunState::Completed as i32
        } else {
            LiveRunState::Ready as i32
        }
        .into();
    } else {
        if v["takeover"]["active"] == true && !stopped && v["session_epoch"].as_u64() == Some(epoch)
        {
            if v["takeover"]["takeover_id"] == x.takeover_id {
                return live::read(tx, root, fence, op, actor, &r.workspace_id, &r.run_id, now);
            }
            return Err(Error::Busy);
        }
        let mut ids = v
            .get("takeover_ids")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if ids.len() >= 32 || ids.iter().any(|i| i == &x.takeover_id) {
            return Err(Error::ResourceExhausted);
        }
        if v["state"] == LiveRunState::Ended as i32 {
            return Err(Error::Cancelled);
        }
        ids.push(x.takeover_id.clone().into());
        v["takeover_ids"] = ids.into();
        let partial =
            v["active_approval"] != "" && v["auto_grant"]["question"]["state"] != "playing";
        let next = epoch.checked_add(1).ok_or(Error::ResourceExhausted)?;
        tx.execute("UPDATE sessions SET epoch=?1,stopped=0,revision=revision+1 WHERE workspace_id=?2 AND id=?3 AND epoch=?4",params![int(next)?,r.workspace_id,r.run_id,int(epoch)?]).wb()?;
        v["session_epoch"] = next.into();
        v["state"] = (LiveRunState::Paused as i32).into();
        if let Some(b) = v.get_mut("batch_approval") {
            b["revoked"] = true.into();
        }
        if let Some(a) = v.get_mut("auto_grant") {
            a["state"] = "stopped".into();
            a["reason"] = "OPERATOR_STOP".into();
            if let Some(q) = a.get_mut("question")
                && matches!(
                    q["state"].as_str(),
                    Some("pending" | "preparing" | "playing")
                )
            {
                q["state"] = "cancelled".into();
            }
        }
        v["takeover"] = json!({"takeover_id":x.takeover_id,"active":true,"partial_sentence":partial,"created_at_ms":now,"resumed_at_ms":0,"epoch":next});
    }
    v["updated_at_ms"] = now.into();
    llm::put(tx, root, &r.workspace_id, &r.run_id, rev, &v, now)?;
    live::read(tx, root, fence, op, actor, &r.workspace_id, &r.run_id, now)
}
