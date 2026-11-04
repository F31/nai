//! Explicit budget policy. Unlimited totals never waive per-call approval/caps.
use nai_contracts::workbench::{WorkbenchError as Error, WorkspaceBudgetMode as Mode};
pub fn validate(mode: i32, limit: u64) -> Result<Mode, Error> {
    let mode = Mode::try_from(mode).map_err(|_| Error::InvalidArgument)?;
    match mode {
        Mode::Disabled | Mode::Unlimited if limit == 0 => Ok(mode),
        Mode::Limited if (1..=1_000_000_000).contains(&limit) => Ok(mode),
        _ => Err(Error::InvalidArgument),
    }
}
#[allow(clippy::too_many_arguments)]
pub fn admit(
    mode: i32,
    limit: u64,
    used: u64,
    unknown: u64,
    reserved: u64,
    next: u64,
    call_cap: u64,
) -> Result<(), Error> {
    let mode = validate(mode, limit)?;
    if next == 0 || next > call_cap || call_cap > 1_000_000_000 {
        return Err(Error::ResourceExhausted);
    }
    let total = used
        .checked_add(unknown)
        .and_then(|v| v.checked_add(reserved))
        .and_then(|v| v.checked_add(next))
        .ok_or(Error::ResourceExhausted)?;
    if mode == Mode::Disabled || mode == Mode::Limited && total > limit {
        return Err(Error::ResourceExhausted);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unspecified_unknown_and_inconsistent_modes_reject() {
        for (mode, limit) in [(0, 0), (99, 0), (1, 1), (2, 0), (3, 1)] {
            assert!(validate(mode, limit).is_err());
        }
    }
    #[test]
    fn unlimited_does_not_waive_call_cap_or_overflow() {
        assert!(admit(3, 0, 100, 20, 30, 5, 5).is_ok());
        assert!(admit(3, 0, 0, 0, 0, 6, 5).is_err());
        assert!(admit(3, 0, u64::MAX, 1, 0, 1, 5).is_err());
    }
    #[test]
    fn limited_counts_unknown_and_reserved_and_disabled_stays_disabled() {
        assert!(admit(2, 100, 40, 30, 20, 10, 10).is_ok());
        assert!(admit(2, 100, 40, 30, 20, 11, 20).is_err());
        assert!(admit(1, 0, 0, 0, 0, 1, 5).is_err());
    }
}
