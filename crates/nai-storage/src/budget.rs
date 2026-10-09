use crate::{
    Operation, Result, Store, error, hash, id,
    store::{access, audit},
};
use nai_budget_core::{Book, BudgetError, Change, Limits};
use nai_contracts::v1::{BillingState, ErrorCode, UsageLedgerEntry};
use prost::Message;
use rusqlite::{Connection, TransactionBehavior, params};

pub struct DispatchClaim {
    pub entry: UsageLedgerEntry,
    pub acquired: bool,
}

fn budget_error(e: BudgetError) -> crate::StorageError {
    error(
        match e {
            BudgetError::Limit | BudgetError::Capacity => ErrorCode::ResourceExhausted,
            _ => ErrorCode::InvalidArgument,
        },
        "budget operation rejected",
    )
}
fn encode(entry: &UsageLedgerEntry) -> Result<String> {
    let bytes = entry.encode_to_vec();
    if bytes.len() > 65536 {
        return Err(error(ErrorCode::InvalidArgument, "oversized ledger entry"));
    }
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(serde_json::json!({"format":"nai.v1.UsageLedgerEntry","protobuf_hex":hex}).to_string())
}
fn decode(text: &str) -> Result<UsageLedgerEntry> {
    if text.len() > 132000 {
        return Err(error(ErrorCode::InvalidArgument, "oversized stored ledger"));
    }
    let json: serde_json::Value = serde_json::from_str(text)
        .map_err(|_| error(ErrorCode::InvalidArgument, "invalid stored ledger"))?;
    if json.get("format").and_then(|v| v.as_str()) != Some("nai.v1.UsageLedgerEntry") {
        return Err(error(
            ErrorCode::InvalidArgument,
            "unsupported stored ledger format",
        ));
    }
    let hex = json
        .get("protobuf_hex")
        .and_then(|v| v.as_str())
        .ok_or_else(|| error(ErrorCode::InvalidArgument, "missing stored ledger bytes"))?;
    if hex.len() % 2 != 0 || hex.len() > 131072 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(error(
            ErrorCode::InvalidArgument,
            "invalid stored ledger bytes",
        ));
    }
    let bytes: Vec<u8> = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|chunk| u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16).unwrap())
        .collect();
    UsageLedgerEntry::decode(bytes.as_slice())
        .map_err(|_| error(ErrorCode::InvalidArgument, "invalid stored ledger protobuf"))
}
fn load(conn: &Connection, workspace: &str, limits: Limits, op: &Operation) -> Result<Book> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM usage_ledger WHERE workspace_id=?1",
        [workspace],
        |r| r.get(0),
    )?;
    if count as usize > limits.max_entries {
        return Err(error(
            ErrorCode::ResourceExhausted,
            "ledger capacity exceeded",
        ));
    }
    let mut book = Book::new(workspace, limits).map_err(budget_error)?;
    let mut query=conn.prepare("SELECT estimate_json,session_id,provider_ref,request_id,billing_state FROM usage_ledger WHERE workspace_id=?1")?;
    let mut rows = query.query([workspace])?;
    let mut entries = Vec::new();
    while let Some(row) = rows.next()? {
        op.check()?;
        let entry = decode(&row.get::<_, String>(0)?)?;
        let state = BillingState::try_from(entry.state)
            .map_err(|_| error(ErrorCode::InvalidArgument, "unknown stored billing state"))?;
        if entry.session_id != row.get::<_, String>(1)?
            || entry.provider_ref != row.get::<_, String>(2)?
            || entry.request_id != row.get::<_, String>(3)?
            || state.as_str_name() != row.get::<_, String>(4)?
        {
            return Err(error(ErrorCode::InvalidArgument, "ledger scope drift"));
        }
        entries.push(entry);
    }
    book.restore_all(entries).map_err(budget_error)?;
    Ok(book)
}
fn write(conn: &Connection, entry: &UsageLedgerEntry) -> Result<()> {
    let ledger_id = hash(
        serde_json::json!([entry.provider_ref, entry.request_id])
            .to_string()
            .as_bytes(),
    );
    let actual=entry.actual.as_ref().map(|a|serde_json::json!({"currency":a.currency,"cost_minor":a.cost_minor.to_string(),"price_revision":a.price_revision,"units":a.units.iter().map(|(k,v)|(k.clone(),v.to_string())).collect::<std::collections::BTreeMap<_,_>>()}).to_string());
    let state = BillingState::try_from(entry.state)
        .map_err(|_| error(ErrorCode::InvalidArgument, "unknown billing state"))?;
    conn.execute("INSERT INTO usage_ledger(workspace_id,ledger_id,session_id,provider_ref,request_id,estimate_json,actual_usage_json,billing_state) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(workspace_id,provider_ref,request_id) DO UPDATE SET estimate_json=excluded.estimate_json,actual_usage_json=excluded.actual_usage_json,billing_state=excluded.billing_state",params![entry.workspace_id,ledger_id,entry.session_id,entry.provider_ref,entry.request_id,encode(entry)?,actual,state.as_str_name()])?;
    Ok(())
}
impl Store {
    /// Limits are trusted owner configuration, never provider/model payload.
    pub fn budget_book(
        &self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        limits: Limits,
    ) -> Result<Book> {
        self.start(op)?;
        access(&self.conn, workspace, actor, "budget.manage", "*")?;
        let book = load(&self.conn, workspace, limits, op)?;
        op.check()?;
        Ok(book)
    }
    pub fn reserve_usage(
        &mut self,
        op: &Operation,
        actor: &str,
        limits: Limits,
        entry: UsageLedgerEntry,
    ) -> Result<UsageLedgerEntry> {
        self.start(op)?;
        id(&entry.workspace_id)?;
        id(&entry.session_id)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        access(&tx, &entry.workspace_id, actor, "budget.manage", "*")?;
        let valid: i64 = tx.query_row(
            "SELECT COUNT(*) FROM sessions WHERE workspace_id=?1 AND session_id=?2",
            params![entry.workspace_id, entry.session_id],
            |r| r.get(0),
        )?;
        if valid != 1 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "budget session scope mismatch",
            ));
        }
        let mut book = load(&tx, &entry.workspace_id, limits, op)?;
        let old = book.get(&entry.provider_ref, &entry.request_id).cloned();
        let result = book.reserve(entry).map_err(budget_error)?;
        if old.as_ref() != Some(&result) {
            write(&tx, &result)?;
            audit(
                &tx,
                &result.workspace_id,
                actor,
                "budget.reserved",
                &result.request_id,
                &serde_json::json!({"revision":result.revision.to_string()}),
            )?;
        }
        op.check()?;
        tx.commit()?;
        Ok(result)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn update_usage(
        &mut self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        limits: Limits,
        provider: &str,
        request: &str,
        expected_revision: u64,
        change: Change,
    ) -> Result<UsageLedgerEntry> {
        if matches!(change, Change::Dispatch) {
            return Err(error(
                ErrorCode::InvalidArgument,
                "dispatch requires an exclusive dispatch claim",
            ));
        }
        self.apply_usage(
            op,
            workspace,
            actor,
            limits,
            provider,
            request,
            expected_revision,
            change,
        )
        .map(|(entry, _)| entry)
    }
    /// Persist the claim BEFORE Provider I/O. Retrying a claimed request returns
    /// acquired=false even if its original ACK was lost; never automatically resend.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_usage(
        &mut self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        limits: Limits,
        provider: &str,
        request: &str,
        expected_revision: u64,
    ) -> Result<DispatchClaim> {
        let (entry, acquired) = self.apply_usage(
            op,
            workspace,
            actor,
            limits,
            provider,
            request,
            expected_revision,
            Change::Dispatch,
        )?;
        Ok(DispatchClaim { entry, acquired })
    }
    #[allow(clippy::too_many_arguments)]
    fn apply_usage(
        &mut self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        limits: Limits,
        provider: &str,
        request: &str,
        expected_revision: u64,
        change: Change,
    ) -> Result<(UsageLedgerEntry, bool)> {
        self.start(op)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        access(&tx, workspace, actor, "budget.manage", "*")?;
        let mut book = load(&tx, workspace, limits, op)?;
        let old = book.get(provider, request).cloned();
        let result = book
            .change(provider, request, expected_revision, change)
            .map_err(budget_error)?;
        let applied = old.as_ref() != Some(&result);
        if applied {
            write(&tx, &result)?;
            audit(
                &tx,
                workspace,
                actor,
                "budget.updated",
                request,
                &serde_json::json!({"revision":result.revision.to_string(),"state":result.state}),
            )?;
        }
        op.check()?;
        tx.commit()?;
        Ok((result, applied))
    }
}
