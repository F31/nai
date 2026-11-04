use super::*;
use nai_runtime_core::foundation::{BudgetPort, Settlement};
impl BudgetPort for Store {
    fn reserve(
        &mut self,
        op: &Operation,
        actor: &str,
        ws: &str,
        id: &str,
        currency: &str,
        amount: u64,
    ) -> Result<()> {
        self.begin(op)?;
        uuid(id)?;
        let current = role(&self.conn, ws, actor)?;
        self.fence.check_scope(&role_scope(ws, actor, current.1))?;
        writer(current.0)?;
        if currency != "CNY" || amount == 0 || amount > 1_000_000_000 {
            return Err(Error::InvalidArgument);
        }
        let now = utc()?;
        let tx = self.conn.transaction().wb()?;
        let prior:Option<(String,u64,String)>=tx.query_row("SELECT principal_id,reserved_minor,currency FROM budget_reservations WHERE workspace_id=?1 AND id=?2",params![ws,id],|r|Ok((r.get(0)?,uint(r,1)?,r.get(2)?))).optional().wb()?;
        if let Some(p) = prior {
            if p != (actor.into(), amount, currency.into()) {
                return Err(Error::RevisionConflict);
            }
            return Ok(());
        }
        llm::admit_budget(&llm::budget(&tx, ws)?, amount, 1_000_000_000)?;
        tx.execute(
            "INSERT INTO budget_reservations VALUES(?1,?2,?3,?4,NULL,'reserved','CNY',1,?5,?5)",
            params![ws, id, actor, int(amount)?, now],
        )
        .wb()?;
        audit(&tx, ws, actor, "budget_reserve", id, now)?;
        op.check()?;
        tx.commit().wb()?;
        Ok(())
    }
    fn settle(
        &mut self,
        op: &Operation,
        actor: &str,
        ws: &str,
        id: &str,
        outcome: Settlement,
    ) -> Result<()> {
        self.begin(op)?;
        uuid(id)?;
        let current = role(&self.conn, ws, actor)?;
        self.fence.check_scope(&role_scope(ws, actor, current.1))?;
        writer(current.0)?;
        let now = utc()?;
        let tx = self.conn.transaction().wb()?;
        let (owner,status,actual):(String,String,Option<u64>)=tx.query_row("SELECT principal_id,status,actual_minor FROM budget_reservations WHERE workspace_id=?1 AND id=?2",params![ws,id],|r|Ok((r.get(0)?,r.get(1)?,uint_opt(r,2)?))).optional().wb()?.ok_or(Error::NotFound)?;
        if owner != actor {
            return Err(Error::PermissionDenied);
        }
        let (next, value) = match outcome {
            Settlement::Known(v) => {
                if v > 1_000_000_000 {
                    return Err(Error::InvalidArgument);
                }
                ("known", Some(v))
            }
            Settlement::Unknown => ("unknown", None),
            Settlement::Released => ("released", None),
        };
        if status == next && actual == value {
            return Ok(());
        }
        if status == "known" || status == "released" || status == "unknown" && next != "known" {
            return Err(Error::RevisionConflict);
        }
        tx.execute("UPDATE budget_reservations SET status=?1,actual_minor=?2,revision=revision+1,updated_at=?3 WHERE workspace_id=?4 AND id=?5",params![next,value.map(int).transpose()?,now,ws,id]).wb()?;
        audit(&tx, ws, actor, "budget_settle", id, now)?;
        op.check()?;
        tx.commit().wb()?;
        Ok(())
    }
}
