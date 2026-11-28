//! Two required transitions plus one coalesced latest authenticated clock.
use nai_contracts::{
    v561_media::{ClockState, PlaybackClockObservation},
    workbench::WorkbenchError as Error,
};
use nai_runtime_core::foundation::Operation;
use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex, mpsc},
    time::{Duration, Instant},
};

pub struct ClockEvent {
    clock: PlaybackClockObservation,
    deadline: Instant,
    operation: Operation,
}
impl ClockEvent {
    /// Metadata alone grants neither frame access nor media execution permission.
    pub fn observation(&self) -> &PlaybackClockObservation {
        &self.clock
    }
    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }
    pub fn running(&self) -> Option<&PlaybackClockObservation> {
        (self.fresh() && self.clock.state == ClockState::Running as i32).then_some(&self.clock)
    }
    /// Device preparation may be fresh without granting running media.
    pub fn fresh(&self) -> bool {
        self.operation.check().is_ok() && Instant::now() <= self.deadline
    }
}
struct State {
    transitions: VecDeque<ClockEvent>,
    latest: Option<ClockEvent>,
    publisher: bool,
    receiver: bool,
    stopped: bool,
}
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}
pub struct ClockPublisher(Arc<Shared>);
pub struct ClockReceiver(Arc<Shared>);
pub fn channel() -> (ClockPublisher, ClockReceiver) {
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            transitions: VecDeque::new(),
            latest: None,
            publisher: true,
            receiver: true,
            stopped: false,
        }),
        wake: Condvar::new(),
    });
    (ClockPublisher(shared.clone()), ClockReceiver(shared))
}
impl ClockPublisher {
    pub(crate) fn publish(
        &self,
        clock: PlaybackClockObservation,
        deadline: Instant,
        operation: Operation,
    ) -> Result<bool, Error> {
        let running = clock.state == ClockState::Running as i32;
        let mut state = self.0.state.lock().map_err(|_| Error::IoFailed)?;
        if !state.receiver {
            return Err(Error::Cancelled);
        }
        if state.stopped {
            return Err(Error::ProtocolError);
        }
        let event = ClockEvent {
            clock,
            deadline,
            operation,
        };
        if running {
            state.latest = Some(event);
        } else {
            if state.transitions.len() == 2 {
                return Err(Error::ResourceExhausted);
            }
            if clock.state == ClockState::Stopped as i32 {
                state.latest = None;
                state.stopped = true;
            }
            state.transitions.push_back(event);
        }
        self.0.wake.notify_one();
        Ok(true)
    }
}
impl Drop for ClockPublisher {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.state.lock() {
            state.publisher = false;
            self.0.wake.notify_all();
        }
    }
}
impl Drop for ClockReceiver {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.state.lock() {
            state.receiver = false;
            state.latest = None;
            state.transitions.clear();
        }
    }
}
impl ClockReceiver {
    pub fn try_recv(&self) -> Result<ClockEvent, mpsc::TryRecvError> {
        let mut state = self
            .0
            .state
            .lock()
            .map_err(|_| mpsc::TryRecvError::Disconnected)?;
        if let Some(event) = state
            .transitions
            .pop_front()
            .or_else(|| state.latest.take())
        {
            return Ok(event);
        }
        Err(if state.publisher {
            mpsc::TryRecvError::Empty
        } else {
            mpsc::TryRecvError::Disconnected
        })
    }
    pub fn recv_timeout(&self, timeout: Duration) -> Result<ClockEvent, mpsc::RecvTimeoutError> {
        let mut state = self
            .0
            .state
            .lock()
            .map_err(|_| mpsc::RecvTimeoutError::Disconnected)?;
        let began = Instant::now();
        loop {
            if let Some(event) = state
                .transitions
                .pop_front()
                .or_else(|| state.latest.take())
            {
                return Ok(event);
            }
            if !state.publisher {
                return Err(mpsc::RecvTimeoutError::Disconnected);
            }
            let Some(left) = timeout.checked_sub(began.elapsed()) else {
                return Err(mpsc::RecvTimeoutError::Timeout);
            };
            state = self
                .0
                .wake
                .wait_timeout(state, left)
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)?
                .0;
        }
    }
}
impl Iterator for ClockReceiver {
    type Item = ClockEvent;
    fn next(&mut self) -> Option<ClockEvent> {
        loop {
            match self.recv_timeout(Duration::from_secs(1)) {
                Ok(event) => return Some(event),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return None,
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn clock(state: ClockState) -> PlaybackClockObservation {
        PlaybackClockObservation {
            epoch: 7,
            device_generation: 1,
            observation_sequence: Some(1),
            state: state as i32,
            played_sample: Some(0),
            sample_rate: 24000,
        }
    }
    #[test]
    fn prepared_freshness_never_grants_running_media() {
        let parent = Operation::model_task(1000).unwrap();
        let event = ClockEvent {
            clock: clock(ClockState::Unavailable),
            deadline: Instant::now() + Duration::from_millis(200),
            operation: parent.clone(),
        };
        assert!(event.fresh());
        assert!(event.running().is_none());
        parent.cancel();
        assert!(!event.fresh());
        assert!(event.running().is_none());
    }
    #[test]
    fn latest_running_replaces_expired_pending_and_transitions_are_preserved() {
        let (tx, rx) = channel();
        let op = Operation::model_task(1000).unwrap();
        let deadline = Instant::now() + Duration::from_millis(200);
        assert!(
            tx.publish(clock(ClockState::Unavailable), deadline, op.clone())
                .unwrap()
        );
        assert!(
            tx.publish(clock(ClockState::Running), deadline, op.clone())
                .unwrap()
        );
        let mut latest = clock(ClockState::Running);
        latest.observation_sequence = Some(99);
        tx.publish(latest, deadline, op.clone()).unwrap();
        assert_eq!(
            rx.try_recv().unwrap().observation().state,
            ClockState::Unavailable as i32
        );
        assert_eq!(
            rx.try_recv().unwrap().observation().observation_sequence,
            Some(99)
        );
        tx.publish(
            clock(ClockState::Running),
            Instant::now() - Duration::from_secs(1),
            op.clone(),
        )
        .unwrap();
        tx.publish(latest, deadline, op.clone()).unwrap();
        assert!(rx.try_recv().unwrap().fresh());
        tx.publish(clock(ClockState::Running), deadline, op.clone())
            .unwrap();
        tx.publish(clock(ClockState::Stopped), deadline, op.clone())
            .unwrap();
        assert_eq!(
            rx.try_recv().unwrap().observation().state,
            ClockState::Stopped as i32
        );
        assert!(rx.try_recv().is_err());
        assert!(
            tx.publish(clock(ClockState::Running), deadline, op.clone())
                .is_err()
        );
        drop(rx);
        assert!(matches!(
            tx.publish(clock(ClockState::Running), deadline, op),
            Err(Error::Cancelled)
        ));
    }
    #[test]
    fn sender_drop_wakes_waiter_without_fabricating_clock() {
        let (tx, rx) = channel();
        let waiter = std::thread::spawn(move || rx.recv_timeout(Duration::from_secs(1)).err());
        drop(tx);
        assert_eq!(
            waiter.join().unwrap(),
            Some(mpsc::RecvTimeoutError::Disconnected)
        );
    }
    #[test]
    fn completed_child_cannot_expose_old_running_before_terminal() {
        let (tx, rx) = channel();
        let op = Operation::model_task(1000).unwrap();
        let deadline = Instant::now() + Duration::from_millis(200);
        tx.publish(clock(ClockState::Running), deadline, op.clone())
            .unwrap();
        tx.publish(clock(ClockState::Stopped), deadline, op.clone())
            .unwrap();
        op.cancel();
        let terminal = rx.try_recv().unwrap();
        assert_eq!(terminal.observation().state, ClockState::Stopped as i32);
        assert!(!terminal.fresh());
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn old_observation_and_cancelled_parent_never_grant_running_clock() {
        let op = Operation::model_task(1000).unwrap();
        let event = ClockEvent {
            clock: clock(ClockState::Running),
            deadline: Instant::now() + Duration::from_millis(200),
            operation: op.clone(),
        };
        assert!(event.running().is_some());
        op.cancel();
        assert!(event.running().is_none());
        let event = ClockEvent {
            clock: clock(ClockState::Running),
            deadline: Instant::now() - Duration::from_millis(1),
            operation: Operation::model_task(1000).unwrap(),
        };
        assert!(event.running().is_none());
    }
}
