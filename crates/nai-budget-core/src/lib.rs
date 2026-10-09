//! Exact shared accounting. A trusted owner supplies limits; no provider I/O.
use nai_contracts::v1::{BillingState, BudgetCharge, UsageLedgerEntry};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetError {
    Invalid,
    UnknownScope,
    UnknownUnit,
    Limit,
    Capacity,
    Conflict,
    Overflow,
}
#[derive(Clone, Debug)]
pub struct Ceiling {
    pub cost_minor: u64,
    pub units: BTreeMap<String, u64>,
    pub concurrent: BTreeMap<String, u64>,
}
#[derive(Clone, Debug)]
pub struct Limits {
    pub currency: String,
    pub global: Ceiling,
    pub sessions: BTreeMap<String, Ceiling>,
    pub providers: BTreeMap<String, Ceiling>,
    pub max_entries: usize,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Total {
    pub cost_minor: u64,
    pub units: BTreeMap<String, u64>,
    pub concurrent: BTreeMap<String, u64>,
}
#[derive(Clone, Debug)]
pub enum Change {
    Dispatch,
    Cancel,
    Finish(Option<BudgetCharge>),
    Reconcile { id: String, actual: BudgetCharge },
}

pub struct Book {
    workspace: String,
    limits: Limits,
    entries: BTreeMap<(String, String), UsageLedgerEntry>,
}
fn id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
fn counters(values: &BTreeMap<String, u64>) -> bool {
    values.len() <= 32
        && values.keys().all(|s| {
            !s.is_empty()
                && s.len() <= 64
                && s.bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
        })
}
fn add(map: &mut BTreeMap<String, u64>, values: &BTreeMap<String, u64>) -> Result<(), BudgetError> {
    for (key, value) in values {
        let old = map.get(key).copied().unwrap_or(0);
        map.insert(
            key.clone(),
            old.checked_add(*value).ok_or(BudgetError::Overflow)?,
        );
    }
    Ok(())
}
impl Book {
    pub fn new(workspace: &str, limits: Limits) -> Result<Self, BudgetError> {
        if !id(workspace)
            || limits.currency.len() != 3
            || !limits.currency.bytes().all(|b| b.is_ascii_uppercase())
            || limits.max_entries == 0
            || limits.max_entries > 100000
            || limits.sessions.len() > 1024
            || limits.providers.len() > 1024
        {
            return Err(BudgetError::Invalid);
        }
        for c in std::iter::once(&limits.global)
            .chain(limits.sessions.values())
            .chain(limits.providers.values())
        {
            if !counters(&c.units) || !counters(&c.concurrent) {
                return Err(BudgetError::Invalid);
            }
        }
        Ok(Self {
            workspace: workspace.into(),
            limits,
            entries: BTreeMap::new(),
        })
    }
    fn validate_charge(
        &self,
        entry: &UsageLedgerEntry,
        charge: &BudgetCharge,
    ) -> Result<(), BudgetError> {
        if charge.currency != self.limits.currency
            || !id(&charge.price_revision)
            || !counters(&charge.units)
        {
            return Err(BudgetError::Invalid);
        }
        // Request admission counts attempts, not provider token billing. A
        // token-only usage response must not erase an already dispatched call.
        if charge.units.get("requests")
            != entry
                .estimate
                .as_ref()
                .and_then(|e| e.units.get("requests"))
        {
            return Err(BudgetError::Invalid);
        }
        let session = self
            .limits
            .sessions
            .get(&entry.session_id)
            .ok_or(BudgetError::UnknownScope)?;
        let provider = self
            .limits
            .providers
            .get(&entry.provider_ref)
            .ok_or(BudgetError::UnknownScope)?;
        for ceiling in [&self.limits.global, session, provider] {
            if charge
                .units
                .keys()
                .any(|key| !ceiling.units.contains_key(key))
                || entry
                    .concurrent
                    .keys()
                    .any(|key| !ceiling.concurrent.contains_key(key))
            {
                return Err(BudgetError::UnknownUnit);
            }
        }
        Ok(())
    }
    fn validate(&self, entry: &UsageLedgerEntry) -> Result<(), BudgetError> {
        if entry.workspace_id != self.workspace
            || !id(&entry.session_id)
            || !id(&entry.provider_ref)
            || !id(&entry.request_id)
            || !counters(&entry.concurrent)
        {
            return Err(BudgetError::Invalid);
        }
        self.validate_charge(entry, entry.estimate.as_ref().ok_or(BudgetError::Invalid)?)?;
        if let Some(actual) = &entry.actual {
            self.validate_charge(entry, actual)?;
        }
        Ok(())
    }
    pub fn entries(&self) -> impl Iterator<Item = &UsageLedgerEntry> {
        self.entries.values()
    }
    pub fn get(&self, provider: &str, request: &str) -> Option<&UsageLedgerEntry> {
        self.entries.get(&(provider.into(), request.into()))
    }
    fn totals(
        &self,
        replacement: Option<&UsageLedgerEntry>,
        filter: impl Fn(&UsageLedgerEntry) -> bool,
    ) -> Result<Total, BudgetError> {
        let mut total = Total::default();
        for entry in self
            .entries
            .values()
            .filter(|old| {
                replacement.is_none_or(|new| {
                    old.provider_ref != new.provider_ref || old.request_id != new.request_id
                })
            })
            .chain(replacement)
        {
            if !filter(entry) || entry.state == BillingState::Released as i32 {
                continue;
            }
            let charge = entry
                .actual
                .as_ref()
                .or(entry.estimate.as_ref())
                .ok_or(BudgetError::Invalid)?;
            total.cost_minor = total
                .cost_minor
                .checked_add(charge.cost_minor)
                .ok_or(BudgetError::Overflow)?;
            add(&mut total.units, &charge.units)?;
            if matches!(
                BillingState::try_from(entry.state),
                Ok(BillingState::Reserved | BillingState::Dispatched)
            ) {
                add(&mut total.concurrent, &entry.concurrent)?;
            }
        }
        Ok(total)
    }
    pub fn total(&self) -> Result<Total, BudgetError> {
        self.totals(None, |_| true)
    }
    fn projected(&self, entry: &UsageLedgerEntry, enforce: bool) -> Result<(), BudgetError> {
        let scope = [
            (&self.limits.global, self.totals(Some(entry), |_| true)?),
            (
                self.limits
                    .sessions
                    .get(&entry.session_id)
                    .ok_or(BudgetError::UnknownScope)?,
                self.totals(Some(entry), |e| e.session_id == entry.session_id)?,
            ),
            (
                self.limits
                    .providers
                    .get(&entry.provider_ref)
                    .ok_or(BudgetError::UnknownScope)?,
                self.totals(Some(entry), |e| e.provider_ref == entry.provider_ref)?,
            ),
        ];
        if enforce
            && scope.iter().any(|(c, t)| {
                t.cost_minor > c.cost_minor
                    || t.units
                        .iter()
                        .any(|(k, v)| *v > c.units.get(k).copied().unwrap_or(0))
                    || t.concurrent
                        .iter()
                        .any(|(k, v)| *v > c.concurrent.get(k).copied().unwrap_or(0))
            })
        {
            return Err(BudgetError::Limit);
        }
        Ok(())
    }
    /// Restoring never frees unresolved reservations, even when limits decreased.
    pub fn restore(&mut self, entry: UsageLedgerEntry) -> Result<(), BudgetError> {
        self.validate_restored(&entry)?;
        if self.entries.len() >= self.limits.max_entries {
            return Err(BudgetError::Capacity);
        }
        let key = (entry.provider_ref.clone(), entry.request_id.clone());
        if self.entries.contains_key(&key) {
            return Err(BudgetError::Conflict);
        }
        self.projected(&entry, false)?;
        self.entries.insert(key, entry);
        Ok(())
    }
    fn validate_restored(&self, entry: &UsageLedgerEntry) -> Result<(), BudgetError> {
        self.validate(entry)?;
        let state = BillingState::try_from(entry.state).map_err(|_| BudgetError::Invalid)?;
        if entry.revision == 0
            || state == BillingState::BillingUnspecified
            || matches!(state, BillingState::Settled | BillingState::Reconciled)
                != entry.actual.is_some()
            || (state == BillingState::Reconciled && !id(&entry.reconciliation_id))
            || (state == BillingState::Released && !entry.cancelled)
            || entry
                .estimate
                .as_ref()
                .is_none_or(|e| e.units.get("requests").copied().unwrap_or(0) == 0)
        {
            return Err(BudgetError::Invalid);
        }
        Ok(())
    }
    /// Batch recovery avoids rescanning the entire book once for every row.
    pub fn restore_all(
        &mut self,
        entries: impl IntoIterator<Item = UsageLedgerEntry>,
    ) -> Result<(), BudgetError> {
        if !self.entries.is_empty() {
            return Err(BudgetError::Conflict);
        }
        let mut recovered = Self::new(&self.workspace, self.limits.clone())?;
        for entry in entries {
            recovered.validate_restored(&entry)?;
            if recovered.entries.len() >= recovered.limits.max_entries {
                return Err(BudgetError::Capacity);
            }
            let key = (entry.provider_ref.clone(), entry.request_id.clone());
            if recovered.entries.insert(key, entry).is_some() {
                return Err(BudgetError::Conflict);
            }
        }
        recovered.total()?;
        self.entries = recovered.entries;
        Ok(())
    }
    pub fn reserve(
        &mut self,
        mut entry: UsageLedgerEntry,
    ) -> Result<UsageLedgerEntry, BudgetError> {
        self.validate(&entry)?;
        if entry.actual.is_some()
            || entry.cancelled
            || !entry.reconciliation_id.is_empty()
            || entry.revision != 0
            || entry.state != BillingState::Reserved as i32
            || entry
                .estimate
                .as_ref()
                .is_none_or(|e| e.units.get("requests").copied().unwrap_or(0) == 0)
        {
            return Err(BudgetError::Invalid);
        }
        if let Some(old) = self.get(&entry.provider_ref, &entry.request_id) {
            if old.session_id != entry.session_id
                || old.generation_epoch != entry.generation_epoch
                || old.estimate != entry.estimate
                || old.concurrent != entry.concurrent
            {
                return Err(BudgetError::Conflict);
            }
            return Ok(old.clone());
        }
        if self.entries.len() >= self.limits.max_entries {
            return Err(BudgetError::Capacity);
        }
        self.projected(&entry, true)?;
        entry.revision = 1;
        self.entries.insert(
            (entry.provider_ref.clone(), entry.request_id.clone()),
            entry.clone(),
        );
        Ok(entry)
    }
    pub fn change(
        &mut self,
        provider: &str,
        request: &str,
        expected_revision: u64,
        change: Change,
    ) -> Result<UsageLedgerEntry, BudgetError> {
        let old = self.get(provider, request).ok_or(BudgetError::Invalid)?;
        let mut next = old.clone();
        let state = BillingState::try_from(old.state).map_err(|_| BudgetError::Invalid)?;
        match change {
            Change::Dispatch => {
                if state == BillingState::Dispatched {
                    return Ok(old.clone());
                }
                if state != BillingState::Reserved || old.cancelled {
                    return Err(BudgetError::Conflict);
                }
                next.state = BillingState::Dispatched as i32;
            }
            Change::Cancel => {
                if old.cancelled {
                    return Ok(old.clone());
                }
                next.cancelled = true;
                if state == BillingState::Reserved {
                    next.state = BillingState::Released as i32;
                }
            }
            Change::Finish(actual) => {
                let desired = if actual.is_some() {
                    BillingState::Settled
                } else {
                    BillingState::NeedsReconciliation
                };
                if state == desired && old.actual == actual {
                    return Ok(old.clone());
                }
                if state != BillingState::Dispatched {
                    return Err(BudgetError::Conflict);
                }
                next.actual = actual;
                next.state = desired as i32;
            }
            Change::Reconcile { id, actual } => {
                if !self::id(&id) {
                    return Err(BudgetError::Invalid);
                }
                if state == BillingState::Reconciled && old.reconciliation_id == id {
                    return if old.actual.as_ref() == Some(&actual) {
                        Ok(old.clone())
                    } else {
                        Err(BudgetError::Conflict)
                    };
                }
                if !matches!(
                    state,
                    BillingState::NeedsReconciliation
                        | BillingState::Settled
                        | BillingState::Reconciled
                ) {
                    return Err(BudgetError::Conflict);
                }
                next.actual = Some(actual);
                next.state = BillingState::Reconciled as i32;
                next.reconciliation_id = id;
            }
        }
        if old.revision != expected_revision {
            return Err(BudgetError::Conflict);
        }
        next.revision = old.revision.checked_add(1).ok_or(BudgetError::Overflow)?;
        self.validate(&next)?;
        self.projected(&next, false)?;
        self.entries
            .insert((provider.into(), request.into()), next.clone());
        Ok(next)
    }
}
