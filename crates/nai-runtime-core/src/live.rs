//! Public queue transitions. Only the composition root can confirm submission.
use crate::foundation::Result;
use nai_contracts::workbench::{LiveRunAction as A, LiveRunState as S, WorkbenchError as E};
/// Fixed from selection; includes boundary wait, synthesis and playback.
/// Network tasks keep their separate 15s cap and inherit any shorter parent.
pub const SELECTED_RESPONSE_WINDOW_MS: i64 = 30_000;
pub fn control(state: S, action: A) -> Result<S> {
    match (state, action) {
        (_, A::End) => Ok(S::Ended),
        (S::Ready | S::Paused, A::Pause) => Ok(S::Paused),
        (S::Dispatching | S::PauseRequested, A::Pause) => Ok(S::PauseRequested),
        (S::Paused | S::Ready, A::Resume) => Ok(S::Ready),
        (S::PauseRequested, A::Resume) => Ok(S::Dispatching),
        _ => Err(E::RevisionConflict),
    }
}
pub fn submitted(state: S, last: bool) -> Result<S> {
    if !matches!(state, S::Dispatching | S::PauseRequested) {
        return Err(E::RevisionConflict);
    }
    Ok(if last {
        S::Completed
    } else if state == S::PauseRequested {
        S::Paused
    } else {
        S::Ready
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pause_waits_for_current_submission_and_resume_does_not_rewind() {
        assert_eq!(control(S::Dispatching, A::Pause), Ok(S::PauseRequested));
        assert_eq!(submitted(S::PauseRequested, false), Ok(S::Paused));
        assert_eq!(control(S::Paused, A::Resume), Ok(S::Ready));
        assert_eq!(submitted(S::Dispatching, true), Ok(S::Completed));
        assert!(control(S::Interrupted, A::Resume).is_err());
        assert!(submitted(S::Ready, false).is_err());
    }
}
