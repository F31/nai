use super::*;
use nai_runtime_core::foundation::{GateInput, delivery_gate};
use serde_json::{Value, json};
fn session(conn: &Connection, ws: &str, id: &str) -> Result<(String, u64, bool)> {
    uuid(id)?;
    conn.query_row(
        "SELECT owner,epoch,stopped FROM sessions WHERE workspace_id=?1 AND id=?2",
        params![ws, id],
        |r| Ok((r.get(0)?, uint(r, 1)?, r.get(2)?)),
    )
    .optional()
    .wb()?
    .ok_or(Error::NotFound)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn submit(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    role_rev: u64,
    tick: u64,
    now: i64,
    x: &FoundationPreview,
    live_index: Option<u32>,
) -> Result<FoundationResult> {
    let snapshot = publication::read(tx, root, fence, op, &x.workspace_id, &x.snapshot_id, now)?;
    if snapshot.publication_status != PublicationStatus::PrivateContent as i32 {
        return Err(Error::CapabilityUnavailable);
    }
    if snapshot.sha256 != x.snapshot_sha256 {
        return Err(Error::RevisionConflict);
    }
    uuid(&x.preview_id)?;
    uuid(&x.session_id)?;
    let manifest: Value =
        serde_json::from_str(&snapshot.body).map_err(|_| Error::IntegrityFailed)?;
    let docs = publication::documents(tx, root, fence, &x.workspace_id, &manifest["refs"], false)?;
    let cue = docs[0]["cues"]
        .as_array()
        .ok_or(Error::IntegrityFailed)?
        .iter()
        .find(|c| c["id"] == x.cue_id)
        .ok_or(Error::NotFound)?;
    let track = docs[1]["talk_tracks"]
        .as_array()
        .ok_or(Error::IntegrityFailed)?
        .iter()
        .find(|t| t["id"] == cue["talk_track_id"])
        .ok_or(Error::IntegrityFailed)?;
    let selection = crate::live::preview_selection(tx, root, fence, op, actor, x, live_index, now)?;
    let text = selection
        .as_ref()
        .map(|s| s.0.as_str())
        .unwrap_or(track["text"].as_str().ok_or(Error::IntegrityFailed)?);
    let epoch = if x.expected_epoch == 0 {
        tx.execute(
            "INSERT INTO sessions VALUES(?1,?2,?3,1,0,1,?4,?4)",
            params![x.workspace_id, x.session_id, actor, now],
        )
        .wb()?;
        1
    } else {
        let (owner, epoch, stopped) = session(tx, &x.workspace_id, &x.session_id)?;
        if owner != actor {
            return Err(Error::PermissionDenied);
        }
        if stopped || epoch != x.expected_epoch {
            return Err(Error::Cancelled);
        }
        epoch
    };
    let mut value = json!({"schema_version":1,"kind":"publication_private_preview","created_at_ms":now,"created_by":actor,"role_revision":role_rev,"snapshot":{"object_id":x.snapshot_id,"revision":1,"sha256":snapshot.sha256},"session_id":x.session_id,"epoch":epoch,"cue_id":x.cue_id,"text":text,"target":"private_workbench","expires_at_ms":now+60000});
    if let Some((_, identity)) = selection {
        value["live_sentence"] = identity;
    }
    let result = publication::insert(tx, root, &x.workspace_id, &x.preview_id, &value, now)?;
    tx.execute(
        "INSERT INTO outbox VALUES(?1,?2,?3,'applied',0,1,?4,?4)",
        params![x.preview_id, x.workspace_id, x.preview_id, now],
    )
    .wb()?;
    let checked = read(
        tx,
        root,
        fence,
        op,
        actor,
        &x.workspace_id,
        &x.preview_id,
        tick,
        now,
    )?;
    if checked.state != DeliveryState::Delivering as i32 {
        return Err(Error::CapabilityUnavailable);
    }
    debug_assert_eq!(checked.sha256, result.sha256);
    Ok(checked)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn read(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    id: &str,
    tick: u64,
    now: i64,
) -> Result<FoundationResult> {
    let (card, sha) = publication::stored(conn, root, ws, id, "publication_private_preview")?;
    if card["created_by"] != actor {
        return Err(Error::PermissionDenied);
    }
    let epoch = card["epoch"].as_u64().ok_or(Error::IntegrityFailed)?;
    let check = (|| {
        let (r, revision) = role(conn, ws, actor)?;
        let (owner, current_epoch, stopped) = session(
            conn,
            ws,
            card["session_id"].as_str().ok_or(Error::IntegrityFailed)?,
        )?;
        if owner != actor {
            return Err(Error::PermissionDenied);
        }
        if stopped || epoch != current_epoch {
            return Err(Error::Cancelled);
        }
        if card.get("live_question").is_some() {
            crate::live_question::check_card(conn, root, fence, op, actor, ws, &card, now)?;
        } else {
            let identity = card.get("live_sentence");
            let request = FoundationPreview {
                workspace_id: ws.into(),
                preview_id: id.into(),
                snapshot_id: card["snapshot"]["object_id"]
                    .as_str()
                    .ok_or(Error::IntegrityFailed)?
                    .into(),
                snapshot_sha256: card["snapshot"]["sha256"]
                    .as_str()
                    .ok_or(Error::IntegrityFailed)?
                    .into(),
                cue_id: card["cue_id"]
                    .as_str()
                    .ok_or(Error::IntegrityFailed)?
                    .into(),
                session_id: card["session_id"]
                    .as_str()
                    .ok_or(Error::IntegrityFailed)?
                    .into(),
                expected_epoch: epoch,
            };
            let selected = crate::live::preview_selection(
                conn,
                root,
                fence,
                op,
                actor,
                &request,
                identity
                    .map(|i| {
                        i["sentence_cursor"]
                            .as_u64()
                            .ok_or(Error::IntegrityFailed)
                            .map(|n| n as u32)
                    })
                    .transpose()?,
                now,
            )?;
            match (selected, identity) {
                (None, None) => (),
                (Some(selected), Some(identity))
                    if selected.1 == *identity && card["text"] == selected.0 => {}
                _ => return Err(Error::RevisionConflict),
            }
        }
        let expires = card["expires_at_ms"]
            .as_i64()
            .ok_or(Error::IntegrityFailed)?;
        if now
            < card["created_at_ms"]
                .as_i64()
                .ok_or(Error::IntegrityFailed)?
            || now >= expires
        {
            return Err(Error::DeadlineExceeded);
        }
        let snap_id = card["snapshot"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?;
        let snap = publication::read(conn, root, fence, op, ws, snap_id, now)?;
        if snap.publication_status != PublicationStatus::PrivateContent as i32
            || snap.sha256 != card["snapshot"]["sha256"]
        {
            return Err(Error::CapabilityUnavailable);
        }
        let role_key = role_scope(ws, actor, revision);
        let scope = object_scope(ws, snap_id);
        let permit = delivery_gate(
            &GateInput {
                role_scope: &role_key,
                object_scope: &scope,
                role: r,
                expected_role_revision: card["role_revision"]
                    .as_u64()
                    .ok_or(Error::IntegrityFailed)?,
                current_role_revision: revision,
                epoch,
                current_epoch,
                stopped,
                object_revision: 1,
                current_object_revision: 1,
                object_sha256: &snap.sha256,
                actual_sha256: &snap.sha256,
                bundle_sha256: &sha,
                approved_sha256: None,
                target: FoundationTarget::PrivateCard,
                scope: DeliveryScope::Private,
                expires_tick: tick
                    + u64::try_from(expires - now).map_err(|_| Error::DeadlineExceeded)?,
                fence_generation: fence.generation(),
                body: card["text"].as_str().ok_or(Error::IntegrityFailed)?,
            },
            fence,
            tick,
            false,
        )?;
        permit.recheck(fence, tick, &sha)
    })();
    let state = match check {
        Ok(()) => DeliveryState::Delivering,
        Err(Error::Cancelled | Error::NotFound) => DeliveryState::Cancelled,
        Err(Error::DeadlineExceeded) => DeliveryState::Expired,
        Err(Error::PermissionDenied | Error::CapabilityUnavailable | Error::RevisionConflict) => {
            DeliveryState::Blocked
        }
        Err(e) => return Err(e),
    };
    let acknowledged:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM audit_events WHERE workspace_id=?1 AND principal_id=?2 AND action='preview_frontend_presented' AND resource_id=?3)",params![ws,actor,id],|r|r.get(0)).wb()?;
    Ok(FoundationResult {
        resource_id: id.into(),
        revision: 1,
        epoch,
        sha256: sha,
        state: state as i32,
        evidence: if acknowledged {
            EvidenceKind::FrontendPresented
        } else {
            EvidenceKind::Submitted
        } as i32,
        body: if state == DeliveryState::Delivering {
            card.to_string()
        } else {
            String::new()
        },
        ..Default::default()
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn presented(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    tick: u64,
    now: i64,
    x: &FoundationPreviewPresented,
) -> Result<FoundationResult> {
    let current = read(
        tx,
        root,
        fence,
        op,
        actor,
        &x.workspace_id,
        &x.preview_id,
        tick,
        now,
    )?;
    if current.sha256 != x.preview_sha256 {
        return Err(Error::RevisionConflict);
    }
    if current.state != DeliveryState::Delivering as i32 {
        return Err(Error::Cancelled);
    }
    let card: Value = serde_json::from_str(&current.body).map_err(|_| Error::IntegrityFailed)?;
    let receipt = json!({"schema_version":1,"kind":"publication_preview_receipt","created_at_ms":now,"created_by":actor,"preview":{"object_id":x.preview_id,"revision":1,"sha256":x.preview_sha256},"session_id":card["session_id"],"epoch":current.epoch});
    publication::insert(tx, root, &x.workspace_id, &x.receipt_id, &receipt, now)?;
    Ok(FoundationResult {
        evidence: EvidenceKind::FrontendPresented as i32,
        ..current
    })
}
pub(super) fn stop(
    tx: &Transaction<'_>,
    actor: &str,
    now: i64,
    x: &FoundationPreviewStop,
) -> Result<FoundationResult> {
    let (owner, epoch, stopped) = session(tx, &x.workspace_id, &x.session_id)?;
    if owner != actor {
        return Err(Error::PermissionDenied);
    }
    if epoch != x.expected_epoch {
        return Err(Error::RevisionConflict);
    }
    if !stopped {
        tx.execute("UPDATE sessions SET stopped=1,epoch=epoch+1,revision=revision+1,updated_at=?1 WHERE workspace_id=?2 AND id=?3",params![now,x.workspace_id,x.session_id]).wb()?;
    }
    Ok(FoundationResult {
        resource_id: x.session_id.clone(),
        epoch: if stopped { epoch } else { epoch + 1 },
        state: DeliveryState::Cancelled as i32,
        ..Default::default()
    })
}
