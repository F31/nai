//! One active task, no pending queue and no automatic restart or resume.
use nai_contracts::workbench::WorkbenchError as Error;
use nai_kernel::output_owner::{OutputLease, OutputOwner, Speaker};
use nai_runtime_core::foundation::Operation;
use std::sync::{Arc, Weak};

/// Receipt is process-local; it is not an IPC command or media execution permit.
pub struct Task {
    identity: Arc<()>,
    operation: Operation,
}
impl Task {
    pub fn operation(&self) -> Operation {
        self.operation.clone()
    }
}

struct Active {
    identity: Arc<()>,
    operation: Operation,
    lease: OutputLease,
}

#[derive(Clone)]
pub struct StopHandle(Weak<OutputOwner>);
impl StopHandle {
    /// Lock-free logical revocation; device completion is a separate receipt.
    pub fn stop(&self) {
        if let Some(owner) = self.0.upgrade() {
            owner.stop();
        }
    }
}

#[derive(Default)]
pub struct Tasks {
    owner: Arc<OutputOwner>,
    active: Option<Active>,
}
impl Tasks {
    pub fn stop_handle(&self) -> StopHandle {
        StopHandle(Arc::downgrade(&self.owner))
    }

    /// The caller serializes control commands; no network or device work runs here.
    /// Validate before replacing the current task so invalid input cannot cancel it.
    pub fn begin(&mut self, speaker: Speaker, deadline_ms: u32) -> Result<Task, Error> {
        let operation = Operation::model_task(deadline_ms)?;
        let lease = self.owner.acquire_lease(speaker).ok_or(Error::Cancelled)?;
        if let Some(old) = self.active.take() {
            old.operation.cancel();
        }
        let check = lease.clone();
        let operation = match operation.bind_permission(Arc::new(move || check.current())) {
            Ok(operation) => operation,
            Err(error) => {
                lease.release();
                return Err(error);
            }
        };
        let identity = Arc::new(());
        self.active = Some(Active {
            identity: identity.clone(),
            operation: operation.clone(),
            lease,
        });
        Ok(Task {
            identity,
            operation,
        })
    }

    /// Validate and retire the exact active task. Err means no result may be used.
    /// Retiring does not automatically start narration or acknowledge media playback.
    pub fn finish(&mut self, task: &Task) -> Result<(), Error> {
        let active = self.active.as_ref().ok_or(Error::Cancelled)?;
        if !Arc::ptr_eq(&active.identity, &task.identity) {
            return Err(Error::Cancelled);
        }
        let result = active.operation.check();
        let active = self.active.take().ok_or(Error::Cancelled)?;
        active.operation.cancel();
        let released = active.lease.release();
        result?;
        if !released {
            return Err(Error::Cancelled);
        }
        Ok(())
    }

    pub fn stop(&mut self) {
        self.owner.stop();
        if let Some(active) = self.active.take() {
            active.operation.cancel();
        }
    }
}
impl Drop for Tasks {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn late_finish_does_not_clear_takeover_and_resume_is_explicit() {
        let mut tasks = Tasks::default();
        let narration = tasks.begin(Speaker::Narration, 4000).unwrap();
        let answer = tasks.begin(Speaker::Answer, 4000).unwrap();
        assert_eq!(narration.operation.check(), Err(Error::Cancelled));
        let human = tasks.begin(Speaker::Human, 4000).unwrap();
        assert!(tasks.finish(&answer).is_err());
        assert!(human.operation.check().is_ok());
        assert!(tasks.begin(Speaker::Narration, 4000).is_err());
        tasks.finish(&human).unwrap();
        assert!(tasks.active.is_none());
        assert!(tasks.begin(Speaker::Narration, 4000).is_ok());
    }

    #[test]
    fn invalid_request_and_cross_room_receipt_cannot_replace_active() {
        let mut first = Tasks::default();
        let mut second = Tasks::default();
        let a = first.begin(Speaker::Answer, 4000).unwrap();
        let b = second.begin(Speaker::Answer, 4000).unwrap();
        assert!(first.begin(Speaker::Human, 0).is_err());
        assert!(first.finish(&b).is_err());
        assert!(a.operation.check().is_ok());
        assert!(b.operation.check().is_ok());
        first.finish(&a).unwrap();
        assert!(first.finish(&a).is_err());
    }

    #[test]
    fn independent_stop_is_terminal_and_drop_invalidates_children() {
        let mut tasks = Tasks::default();
        let task = tasks.begin(Speaker::Answer, 4000).unwrap();
        let child = task.operation.model_child(15000).unwrap();
        let stop = tasks.stop_handle();
        std::thread::spawn(move || stop.stop()).join().unwrap();
        assert_eq!(child.check(), Err(Error::Cancelled));
        assert!(tasks.finish(&task).is_err());
        assert!(tasks.begin(Speaker::Human, 4000).is_err());
        let mut fresh = Tasks::default();
        let task = fresh.begin(Speaker::Narration, 4000).unwrap();
        drop(fresh);
        assert_eq!(task.operation.check(), Err(Error::Cancelled));
    }

    #[test]
    fn expired_task_releases_slot_without_automatically_resuming() {
        let mut tasks = Tasks::default();
        let task = tasks.begin(Speaker::Answer, 1).unwrap();
        std::thread::sleep(Duration::from_millis(10));
        assert_eq!(tasks.finish(&task), Err(Error::DeadlineExceeded));
        assert!(tasks.active.is_none());
        assert!(tasks.begin(Speaker::Narration, 4000).is_ok());
    }

    #[test]
    fn assembly_takeover_cancels_real_http_wait_and_rejects_late_finish() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", server.server_addr());
        let mut tasks = Tasks::default();
        let task = tasks.begin(Speaker::Narration, 4000).unwrap();
        let operation = task.operation().model_child(15000).unwrap();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            let out = nai_llm_openai::qwen::synthesize(
                &nai_llm_openai::qwen::Config {
                    endpoint: url,
                    deadline_ms: 4000,
                    ..Default::default()
                },
                "你好",
                operation,
            );
            tx.send((out.error, out.samples, out.request_started))
                .unwrap();
        });
        let held = server
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .expect("HTTP sent");
        let human = tasks.begin(Speaker::Human, 4000).unwrap();
        let (error, samples, sent) = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(error, Some(Error::Cancelled));
        assert!(samples.is_empty());
        assert!(sent);
        assert!(tasks.finish(&task).is_err());
        assert!(human.operation.check().is_ok());
        tasks.finish(&human).unwrap();
        worker.join().unwrap();
        drop(held);
        assert!(
            server
                .recv_timeout(Duration::from_millis(50))
                .unwrap()
                .is_none()
        );
    }
}
