use crate::client::ProviderError;
use nai_budget_core::{Book, BudgetError, Change, Limits};
use nai_contracts::v1::{BudgetCharge, ErrorCode, UsageLedgerEntry};

/// Wiring boundary implemented by the composition root against `Store`
/// (D07 durable ledger). Tests use [`InMemoryGateway`].
pub trait UsageGateway: Send {
    fn reserve(&mut self, entry: UsageLedgerEntry) -> Result<UsageLedgerEntry, ProviderError>;
    fn dispatch(
        &mut self,
        provider: &str,
        request: &str,
        revision: u64,
    ) -> Result<DispatchDecision, ProviderError>;
    fn finish(
        &mut self,
        provider: &str,
        request: &str,
        revision: u64,
        actual: Option<BudgetCharge>,
    ) -> Result<UsageLedgerEntry, ProviderError>;
}

#[derive(Debug, Clone)]
pub enum DispatchDecision {
    /// This call obtained the exclusive claim and MAY start provider I/O.
    Acquired(UsageLedgerEntry),
    /// The request was already dispatched (e.g. ACK retry). Do NOT re-send.
    Already(UsageLedgerEntry),
}

fn map_budget(error: BudgetError) -> ProviderError {
    let code = match error {
        BudgetError::Limit | BudgetError::Capacity => ErrorCode::ResourceExhausted,
        BudgetError::Conflict => ErrorCode::Conflict,
        BudgetError::Overflow => ErrorCode::InvalidArgument,
        _ => ErrorCode::InvalidArgument,
    };
    ProviderError::new(code, false, "budget reservation rejected")
}

/// Test double backed by an exact in-memory `Book`. Production wiring uses the
/// durable SQLite ledger via the same trait.
pub struct InMemoryGateway {
    book: Book,
}
impl InMemoryGateway {
    pub fn new(workspace: &str, limits: Limits) -> Result<Self, BudgetError> {
        Ok(Self {
            book: Book::new(workspace, limits)?,
        })
    }
    pub fn book(&self) -> &Book {
        &self.book
    }
}
impl UsageGateway for InMemoryGateway {
    fn reserve(&mut self, entry: UsageLedgerEntry) -> Result<UsageLedgerEntry, ProviderError> {
        self.book.reserve(entry).map_err(map_budget)
    }
    fn dispatch(
        &mut self,
        provider: &str,
        request: &str,
        revision: u64,
    ) -> Result<DispatchDecision, ProviderError> {
        let state = self
            .book
            .get(provider, request)
            .map(|entry| nai_contracts::v1::BillingState::try_from(entry.state));
        match state {
            Some(Ok(nai_contracts::v1::BillingState::Reserved)) => {
                let entry = self
                    .book
                    .change(provider, request, revision, Change::Dispatch)
                    .map_err(map_budget)?;
                Ok(DispatchDecision::Acquired(entry))
            }
            // Already dispatched, or already settled/released: never re-send.
            Some(Ok(_)) => Ok(DispatchDecision::Already(
                self.book.get(provider, request).unwrap().clone(),
            )),
            Some(Err(_)) | None => Err(ProviderError::new(
                ErrorCode::Conflict,
                false,
                "request not reserved",
            )),
        }
    }
    fn finish(
        &mut self,
        provider: &str,
        request: &str,
        revision: u64,
        actual: Option<BudgetCharge>,
    ) -> Result<UsageLedgerEntry, ProviderError> {
        let change = if actual.is_some() {
            Change::Finish(actual)
        } else {
            Change::Finish(None)
        };
        self.book
            .change(provider, request, revision, change)
            .map_err(map_budget)
    }
}

/// Reserve → dispatch → finish around one provider call. Provider I/O is only
/// started when `start` reports `dispatched()`; retries return Already and skip.
pub struct CallBudget {
    provider: String,
    request: String,
    revision: u64,
    dispatched: bool,
}
impl CallBudget {
    pub fn start(
        gateway: &mut dyn UsageGateway,
        entry: UsageLedgerEntry,
    ) -> Result<Self, ProviderError> {
        let reserved = gateway.reserve(entry)?;
        let provider = reserved.provider_ref.clone();
        let request = reserved.request_id.clone();
        let decision = gateway.dispatch(&provider, &request, reserved.revision)?;
        let (dispatched, revision) = match decision {
            // Dispatch advanced the stored revision; finish must CAS against it.
            DispatchDecision::Acquired(entry) => (true, entry.revision),
            DispatchDecision::Already(_) => (false, reserved.revision),
        };
        Ok(Self {
            provider,
            request,
            revision,
            dispatched,
        })
    }
    pub fn dispatched(&self) -> bool {
        self.dispatched
    }
    /// Records the final state. Call this on success, error and cancellation;
    /// `actual=None` keeps the estimate for reconciliation.
    pub fn finish(
        self,
        gateway: &mut dyn UsageGateway,
        actual: Option<BudgetCharge>,
    ) -> Result<(), ProviderError> {
        if self.dispatched {
            gateway.finish(&self.provider, &self.request, self.revision, actual)?;
        }
        Ok(())
    }
}
