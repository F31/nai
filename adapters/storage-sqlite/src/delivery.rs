use super::*;
use nai_runtime_core::foundation::{GateInput, Permit, delivery_gate};
struct Delivery {
    session: String,
    epoch: u64,
    object: String,
    revision: u64,
    sha: String,
    bundle: String,
    recipient: String,
    role_revision: u64,
    expires: u64,
    generation: u64,
    state: i32,
    evidence: i32,
}
fn load(conn: &Connection, ws: &str, id: &str) -> Result<Delivery> {
    uuid(ws)?;
    uuid(id)?;
    conn.query_row("SELECT session_id,epoch,object_id,object_revision,object_sha256,bundle_sha256,recipient,role_revision,expires_tick,fence_generation,state,evidence FROM deliveries WHERE workspace_id=?1 AND id=?2",params![ws,id],|r|Ok(Delivery{session:r.get(0)?,epoch:uint(r,1)?,object:r.get(2)?,revision:uint(r,3)?,sha:r.get(4)?,bundle:r.get(5)?,recipient:r.get(6)?,role_revision:uint(r,7)?,expires:uint(r,8)?,generation:uint(r,9)?,state:r.get(10)?,evidence:r.get(11)?})).optional().wb()?.ok_or(Error::NotFound)
}
// Keep the complete transaction snapshot and trusted caller explicit at this private boundary.
#[allow(clippy::too_many_arguments)]
fn gate(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    tick: u64,
    actor: &str,
    ws: &str,
    id: &str,
    d: &Delivery,
    approved: bool,
) -> Result<Permit> {
    if d.recipient != actor {
        return Err(Error::PermissionDenied);
    }
    let (role, rev) = super::role(conn, ws, actor)?;
    let (epoch, stopped): (u64, bool) = conn
        .query_row(
            "SELECT epoch,stopped FROM sessions WHERE workspace_id=?1 AND id=?2",
            params![ws, d.session],
            |r| Ok((uint(r, 0)?, r.get(1)?)),
        )
        .optional()
        .wb()?
        .ok_or(Error::NotFound)?;
    let (current, actual) = object(conn, ws, &d.object, 0)?;
    let text = body(root, &d.sha)?;
    if nai_runtime_core::preparation::kind(&text).is_some() {
        return Err(Error::CapabilityUnavailable);
    }
    let approval:Option<String>=conn.query_row("SELECT bundle_sha256 FROM approvals WHERE workspace_id=?1 AND delivery_id=?2 AND principal_id=?3 AND role_revision=?4",params![ws,id,actor,int(rev)?],|r|r.get(0)).optional().wb()?;
    let permit = delivery_gate(
        &GateInput {
            role_scope: &role_scope(ws, actor, rev),
            object_scope: &object_scope(ws, &d.object),
            role,
            expected_role_revision: d.role_revision,
            current_role_revision: rev,
            epoch: d.epoch,
            current_epoch: epoch,
            stopped,
            object_revision: d.revision,
            current_object_revision: current,
            object_sha256: &d.sha,
            actual_sha256: &actual,
            bundle_sha256: &d.bundle,
            approved_sha256: approval.as_deref(),
            target: FoundationTarget::PrivateCard,
            scope: DeliveryScope::Private,
            expires_tick: d.expires,
            fence_generation: d.generation,
            body: &text,
        },
        fence,
        tick,
        approved,
    )?;
    permit.recheck(fence, tick, &d.bundle)?;
    Ok(permit)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    tick: u64,
    now: i64,
    actor: &str,
    role_rev: u64,
    x: &FoundationDelivery,
) -> Result<FoundationResult> {
    for id in [&x.delivery_id, &x.session_id, &x.object_id] {
        uuid(id)?;
    }
    if x.target != FoundationTarget::PrivateCard as i32 || x.scope != DeliveryScope::Private as i32
    {
        return Err(Error::CapabilityUnavailable);
    }
    if !(1..=30000).contains(&x.ttl_ms) {
        return Err(Error::InvalidArgument);
    }
    fence.check(fence.generation())?;
    let (epoch, stopped, owner): (u64, bool, String) = tx
        .query_row(
            "SELECT epoch,stopped,owner FROM sessions WHERE workspace_id=?1 AND id=?2",
            params![x.workspace_id, x.session_id],
            |r| Ok((uint(r, 0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .wb()?
        .ok_or(Error::NotFound)?;
    if stopped {
        return Err(Error::Cancelled);
    }
    if owner != actor {
        return Err(Error::PermissionDenied);
    }
    let (current, sha) = object(tx, &x.workspace_id, &x.object_id, 0)?;
    if current != x.object_revision || sha != x.object_sha256 {
        return Err(Error::RevisionConflict);
    }
    let text = body(root, &sha)?;
    manual_guard(&text)?;
    if nai_runtime_core::preparation::kind(&text).is_some() {
        return Err(Error::CapabilityUnavailable);
    }
    tx.execute("UPDATE deliveries SET state=?1,revision=revision+1,updated_at=?2 WHERE expires_tick<=?3 AND state IN (?4,?5)",params![DeliveryState::Expired as i32,now,int(tick)?,DeliveryState::AwaitingApproval as i32,DeliveryState::Approved as i32]).wb()?;
    tx.execute("UPDATE outbox SET state='cancelled',revision=revision+1,updated_at=?1 WHERE state='pending' AND EXISTS(SELECT 1 FROM deliveries d WHERE d.workspace_id=outbox.workspace_id AND d.id=outbox.resource_id AND d.state=?2)",params![now,DeliveryState::Expired as i32]).wb()?;
    let pending: u32 = tx
        .query_row(
            "SELECT COUNT(*) FROM outbox WHERE state IN ('pending','dispatching')",
            [],
            |r| r.get(0),
        )
        .wb()?;
    if pending >= 16 {
        return Err(Error::ResourceExhausted);
    }
    let mut bytes = x.encode_to_vec();
    bytes.extend_from_slice(actor.as_bytes());
    bytes.extend_from_slice(&epoch.to_le_bytes());
    bytes.extend_from_slice(&role_rev.to_le_bytes());
    let bundle = hash(&bytes);
    tx.execute(
        "INSERT INTO deliveries VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,0,1,?14,?14)",
        params![
            x.workspace_id,
            x.delivery_id,
            x.session_id,
            int(epoch)?,
            x.object_id,
            int(current)?,
            sha,
            bundle,
            actor,
            int(role_rev)?,
            int(tick + u64::from(x.ttl_ms))?,
            int(fence.generation())?,
            DeliveryState::AwaitingApproval as i32,
            now
        ],
    )
    .wb()?;
    tx.execute(
        "INSERT INTO outbox VALUES(?1,?2,?3,'pending',0,1,?4,?4)",
        params![
            format!("{}:{}", x.workspace_id, x.delivery_id),
            x.workspace_id,
            x.delivery_id,
            now
        ],
    )
    .wb()?;
    Ok(FoundationResult {
        resource_id: x.delivery_id.clone(),
        revision: 1,
        epoch,
        sha256: bundle,
        state: DeliveryState::AwaitingApproval as i32,
        ..Default::default()
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn approve(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    tick: u64,
    now: i64,
    actor: &str,
    x: &FoundationApprove,
) -> Result<FoundationResult> {
    let d = load(tx, &x.workspace_id, &x.delivery_id)?;
    if d.state != DeliveryState::AwaitingApproval as i32
        && d.state != DeliveryState::Approved as i32
    {
        return Err(Error::RevisionConflict);
    }
    if x.bundle_sha256 != d.bundle {
        return Err(Error::IntegrityFailed);
    }
    let permit = gate(
        tx,
        root,
        fence,
        tick,
        actor,
        &x.workspace_id,
        &x.delivery_id,
        &d,
        false,
    )?;
    tx.execute("INSERT INTO approvals VALUES(?1,?2,?3,?4,?5,1,?6,?6) ON CONFLICT(workspace_id,delivery_id,principal_id) DO NOTHING",params![x.workspace_id,x.delivery_id,actor,d.bundle,int(d.role_revision)?,now]).wb()?;
    tx.execute("UPDATE deliveries SET state=?1,revision=revision+1,updated_at=?2 WHERE workspace_id=?3 AND id=?4 AND state=?5",params![DeliveryState::Approved as i32,now,x.workspace_id,x.delivery_id,DeliveryState::AwaitingApproval as i32]).wb()?;
    permit.recheck(fence, tick, &d.bundle)?;
    read(tx, root, &x.workspace_id, &x.delivery_id, actor, fence)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn claim(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    tick: u64,
    now: i64,
    actor: &str,
    x: &FoundationDispatch,
) -> Result<FoundationResult> {
    fence.check(fence.generation())?;
    let d = load(tx, &x.workspace_id, &x.delivery_id)?;
    if d.state == DeliveryState::OutcomeUnknown as i32 {
        return Err(Error::OutcomeUnknown);
    }
    if d.state != DeliveryState::Approved as i32 {
        return Err(Error::RevisionConflict);
    }
    let permit = gate(
        tx,
        root,
        fence,
        tick,
        actor,
        &x.workspace_id,
        &x.delivery_id,
        &d,
        true,
    )?;
    let changed=tx.execute("UPDATE outbox SET state='dispatching',attempts=1,revision=revision+1,updated_at=?1 WHERE workspace_id=?2 AND resource_id=?3 AND state='pending' AND attempts=0",params![now,x.workspace_id,x.delivery_id]).wb()?;
    if changed != 1 {
        return Err(Error::OutcomeUnknown);
    }
    tx.execute("UPDATE deliveries SET state=?1,revision=revision+1,updated_at=?2 WHERE workspace_id=?3 AND id=?4",params![DeliveryState::Committed as i32,now,x.workspace_id,x.delivery_id]).wb()?;
    permit.recheck(fence, tick, &d.bundle)?;
    read(tx, root, &x.workspace_id, &x.delivery_id, actor, fence)
}
pub(super) fn read(
    conn: &Connection,
    root: &Path,
    ws: &str,
    id: &str,
    actor: &str,
    fence: &StopFence,
) -> Result<FoundationResult> {
    let d = load(conn, ws, id)?;
    super::role(conn, ws, actor)?;
    if d.recipient != actor {
        return Err(Error::PermissionDenied);
    }
    let present:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM private_cards WHERE workspace_id=?1 AND delivery_id=?2 AND recipient=?3)",params![ws,id,actor],|r|r.get(0)).wb()?;
    let deleted: bool = conn
        .query_row(
            "SELECT deleted FROM objects WHERE workspace_id=?1 AND id=?2",
            params![ws, d.object],
            |r| r.get(0),
        )
        .wb()?;
    let revoked = fence.check_scope(&object_scope(ws, &d.object)).is_err();
    Ok(FoundationResult {
        resource_id: id.into(),
        epoch: d.epoch,
        sha256: d.bundle,
        revision: conn
            .query_row(
                "SELECT revision FROM deliveries WHERE workspace_id=?1 AND id=?2",
                params![ws, id],
                |r| uint(r, 0),
            )
            .wb()?,
        state: if revoked
            && [
                DeliveryState::AwaitingApproval as i32,
                DeliveryState::Approved as i32,
                DeliveryState::Committed as i32,
            ]
            .contains(&d.state)
        {
            DeliveryState::Blocked as i32
        } else {
            d.state
        },
        evidence: d.evidence,
        body: if present && !deleted && !revoked {
            body(root, &d.sha)?
        } else {
            String::new()
        },
        ..Default::default()
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn project(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    tick: u64,
    now: i64,
    actor: &str,
    ws: &str,
    id: &str,
) -> Result<FoundationResult> {
    let d = load(tx, ws, id)?;
    if d.state == DeliveryState::Delivering as i32 {
        return read(tx, root, ws, id, actor, fence);
    }
    if d.state == DeliveryState::OutcomeUnknown as i32 {
        return Err(Error::OutcomeUnknown);
    }
    if d.state != DeliveryState::Committed as i32 {
        return Err(Error::RevisionConflict);
    }
    let permit = gate(tx, root, fence, tick, actor, ws, id, &d, true)?;
    tx.execute(
        "INSERT INTO private_cards VALUES(?1,?2,?3,?4,1,?5,?5)",
        params![ws, id, actor, d.sha, now],
    )
    .wb()?;
    tx.execute("UPDATE deliveries SET state=?1,evidence=?2,revision=revision+1,updated_at=?3 WHERE workspace_id=?4 AND id=?5",params![DeliveryState::Delivering as i32,EvidenceKind::Submitted as i32,now,ws,id]).wb()?;
    tx.execute("UPDATE outbox SET state='applied',revision=revision+1,updated_at=?1 WHERE workspace_id=?2 AND resource_id=?3 AND state='dispatching'",params![now,ws,id]).wb()?;
    audit(tx, ws, actor, "private_card_submitted", id, now)?;
    permit.recheck(fence, tick, &d.bundle)?;
    read(tx, root, ws, id, actor, fence)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_current(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    tick: u64,
    actor: &str,
    ws: &str,
    id: &str,
    approved: bool,
) -> Result<()> {
    let d = load(conn, ws, id)?;
    gate(conn, root, fence, tick, actor, ws, id, &d, approved)?;
    Ok(())
}
