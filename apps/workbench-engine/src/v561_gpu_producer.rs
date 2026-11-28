//! Single model owner for one script, at most one sentence ahead of playback.
use crate::{
    v561_avatar_stream::Resident, v561_script_source::Pipeline, v561_stream_clip::Clip,
    v561_stream_synthesis,
};
use nai_contracts::workbench::WorkbenchError as Error;
use nai_runtime_core::foundation::Operation;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
        mpsc,
    },
    time::Duration,
};
pub type Outcome = (Resident, u64, Result<(), Error>);
pub struct Producer {
    rx: mpsc::Receiver<Result<Clip, Error>>,
    root: Operation,
    worker: Option<std::thread::JoinHandle<Outcome>>,
    seen: u32,
    count: u32,
}
impl Producer {
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        mut owner: Resident,
        pipeline: Arc<Mutex<Pipeline>>,
        root: Operation,
        mut epoch: u64,
        count: u32,
        directory: PathBuf,
        pixel_bytes: u64,
        trace: PathBuf,
    ) -> Self {
        let (tx, rx) = mpsc::sync_channel(1);
        let op = root.clone();
        let promoted = Arc::new(AtomicU32::new(0));
        let worker = std::thread::spawn(move || {
            let result = (|| -> Result<(), Error> {
                for index in 1..=count {
                    while index > 1 && promoted.load(Ordering::Acquire) < index - 1 {
                        op.check()?;
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    let source = pipeline
                        .lock()
                        .map_err(|_| Error::IoFailed)?
                        .next_source_with(|| op.check())?
                        .ok_or(Error::ProtocolError)?;
                    epoch = epoch.checked_add(1).ok_or(Error::ProtocolError)?;
                    let (clip, mut writer) = Clip::channel(
                        &source,
                        epoch,
                        &directory,
                        pixel_bytes,
                        op.clone(),
                        promoted.clone(),
                    )?;
                    std::fs::create_dir_all(&trace).map_err(|_| Error::IoFailed)?;
                    let event = serde_json::json!({"phase":"source_generation_started","epoch":epoch,"sentence_index":index});
                    serde_json::to_writer(
                        std::fs::File::create_new(trace.join(format!("segment-{epoch}.json")))
                            .map_err(|_| Error::IoFailed)?,
                        &event,
                    )
                    .map_err(|_| Error::IoFailed)?;
                    let task = op.model_child(op.remaining_ms()?)?;
                    let mut pending = Ok(clip);
                    loop {
                        op.check()?;
                        match tx.try_send(pending) {
                            Ok(()) => break,
                            Err(mpsc::TrySendError::Full(v)) => pending = v,
                            Err(_) => return Err(Error::Cancelled),
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    let maximum = source.max_samples;
                    let result = v561_stream_synthesis::generate_source(
                        &mut owner,
                        maximum,
                        epoch,
                        epoch,
                        task,
                        &mut writer,
                        move |child, push| source.consume(child, push),
                    );
                    writer.finish(result)?;
                }
                Ok(())
            })();
            if let Err(e) = result {
                let _ = tx.try_send(Err(e));
            }
            (owner, epoch, result)
        });
        Self {
            rx,
            root,
            worker: Some(worker),
            seen: 0,
            count,
        }
    }
    pub fn next_with(
        &mut self,
        mut idle: impl FnMut() -> Result<(), Error>,
    ) -> Result<Option<Clip>, Error> {
        if self.seen == self.count {
            return Ok(None);
        }
        loop {
            self.root.check()?;
            idle()?;
            match self.rx.recv_timeout(Duration::from_millis(10)) {
                Ok(result) => {
                    let clip = result?;
                    self.seen += 1;
                    if clip.facts.index != self.seen {
                        return Err(Error::ProtocolError);
                    }
                    return Ok(Some(clip));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err(Error::OutcomeUnknown),
            }
        }
    }
    pub fn finish(mut self, cancel: bool) -> Result<(Resident, u64, Result<(), Error>), Error> {
        if cancel {
            self.root.cancel();
        }
        self.worker
            .take()
            .ok_or(Error::ProtocolError)?
            .join()
            .map_err(|_| Error::OutcomeUnknown)
    }
}
impl Drop for Producer {
    fn drop(&mut self) {
        if let Some(t) = self.worker.take() {
            self.root.cancel();
            let _ = t.join();
        }
    }
}
