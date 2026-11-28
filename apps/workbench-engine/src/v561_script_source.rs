//! One TTS producer: streaming current source and exactly one future PCM slot.
use nai_contracts::workbench::WorkbenchError as Error;
use nai_llm_openai::cosyvoice::{self, Config, Summary};
use nai_runtime_core::foundation::Operation;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;
enum Part {
    Data(Zeroizing<Vec<u8>>),
    End(Result<Summary, Error>),
}
enum Body {
    Stream(mpsc::Receiver<Part>),
}
pub struct Source {
    body: Body,
    root: Operation,
    pub max_samples: u64,
    pub index: u32,
    pub origin: Instant,
    pub ready_at: Option<Instant>,
    verified: Option<mpsc::SyncSender<u32>>,
    claimed: Arc<std::sync::atomic::AtomicBool>,
}
pub(crate) struct PrefetchPermit {
    sender: Option<mpsc::SyncSender<u32>>,
    root: Operation,
    index: u32,
    claimed: Arc<std::sync::atomic::AtomicBool>,
}
impl PrefetchPermit {
    pub(crate) fn ready(self) -> Result<(), Error> {
        self.root.check()?;
        if self.claimed.swap(true, Ordering::AcqRel) {
            return Err(Error::ProtocolError);
        }
        if let Some(tx) = self.sender {
            tx.try_send(self.index).map_err(|_| Error::ProtocolError)?;
        }
        Ok(())
    }
}
fn send<T>(tx: &mpsc::SyncSender<T>, mut value: T, op: &Operation) -> Result<(), Error> {
    loop {
        op.check()?;
        match tx.try_send(value) {
            Ok(()) => return Ok(()),
            Err(mpsc::TrySendError::Full(v)) => value = v,
            Err(_) => return Err(Error::Cancelled),
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
impl Source {
    #[cfg(test)]
    pub(crate) fn test_empty(root: Operation, maximum: u64) -> Self {
        let (_tx, rx) = mpsc::sync_channel(1);
        Self {
            body: Body::Stream(rx),
            root,
            max_samples: maximum,
            index: 1,
            origin: Instant::now(),
            ready_at: None,
            verified: None,
            claimed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
    pub(crate) fn prefetch_permit(&self) -> PrefetchPermit {
        PrefetchPermit {
            sender: self.verified.clone(),
            root: self.root.clone(),
            index: self.index,
            claimed: self.claimed.clone(),
        }
    }
    pub fn consume(
        self,
        operation: Operation,
        mut accept: impl FnMut(&[u8]) -> Result<(), Error>,
    ) -> Result<Summary, Error> {
        let check = || {
            self.root.check()?;
            operation.check()
        };
        check()?;
        match self.body {
            Body::Stream(rx) => {
                let mut samples = 0u64;
                loop {
                    check()?;
                    match rx.recv_timeout(Duration::from_millis(10)) {
                        Ok(Part::Data(bytes)) => {
                            if bytes.is_empty()
                                || !bytes.len().is_multiple_of(2)
                                || bytes.len() > 9600
                                || bytes.len() as u64 / 2 > self.max_samples - samples
                            {
                                return Err(Error::ProtocolError);
                            }
                            samples += bytes.len() as u64 / 2;
                            accept(&bytes)?;
                        }
                        Ok(Part::End(result)) => {
                            let summary = result?;
                            check()?;
                            if samples == 0 || samples != summary.samples {
                                return Err(Error::ProtocolError);
                            }
                            return Ok(summary);
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(_) => return Err(Error::OutcomeUnknown),
                    }
                }
            }
        }
    }
}
pub struct Pipeline {
    first: Option<Source>,
    ready: mpsc::Receiver<Result<Source, Error>>,
    permit: mpsc::SyncSender<()>,
    root: Operation,
    worker: Option<JoinHandle<()>>,
    pub count: u32,
    taken: u32,
    calls: Arc<AtomicU64>,
    pub origin: Instant,
}
impl Pipeline {
    pub fn new(config: Config, text: &str, root: Operation) -> Result<Self, Error> {
        Self::start(config, text, root, |config, text, op, accept| {
            cosyvoice::stream(config, text, op, accept)
        })
    }
    fn start(
        config: Config,
        text: &str,
        root: Operation,
        mut synth: impl FnMut(
            &Config,
            &str,
            Operation,
            &mut dyn FnMut(&[u8]) -> Result<(), Error>,
        ) -> Result<Summary, Error>
        + Send
        + 'static,
    ) -> Result<Self, Error> {
        root.check()?;
        let parts = config.lookahead.split(text)?;
        let count = parts.len() as u32;
        let mut config = config;
        config.max_samples = config
            .max_samples
            .min(config.lookahead.max_sentence_samples)
            .min(144000);
        if config.max_samples == 0 {
            return Err(Error::InvalidArgument);
        }
        let origin = Instant::now();
        let (tx, rx) = mpsc::sync_channel(16);
        let (ready_tx, ready) = mpsc::sync_channel(1);
        let (permit, permit_rx) = mpsc::sync_channel(1);
        let (verified, verified_rx) = mpsc::sync_channel(1);
        let calls = Arc::new(AtomicU64::new(0));
        let worker_calls = calls.clone();
        let worker_root = root.clone();
        let first = Source {
            body: Body::Stream(rx),
            root: root.clone(),
            max_samples: config.max_samples,
            index: 1,
            origin,
            ready_at: None,
            verified: if count > 1 {
                Some(verified.clone())
            } else {
                None
            },
            claimed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        let worker = std::thread::Builder::new()
            .name("single-tts-lookahead".into())
            .spawn(move || {
                let result = (|| -> Result<(), Error> {
                    worker_root.check()?;
                    worker_calls.fetch_add(1, Ordering::AcqRel);
                    let first_result = synth(
                        &config,
                        &parts[0],
                        worker_root.model_child(config.deadline_ms)?,
                        &mut |bytes| {
                            for chunk in bytes.chunks(9600) {
                                send(
                                    &tx,
                                    Part::Data(Zeroizing::new(chunk.to_vec())),
                                    &worker_root,
                                )?;
                            }
                            Ok(())
                        },
                    );
                    let failed = first_result.is_err();
                    send(&tx, Part::End(first_result), &worker_root)?;
                    if failed {
                        return Ok(());
                    }
                    for (index, text) in parts.iter().enumerate().skip(1) {
                        // Current verified source releases this permit once;
                        // the next TTS can overlap remaining GPU/device work.
                        loop {
                            worker_root.check()?;
                            match verified_rx.recv_timeout(Duration::from_millis(10)) {
                                Ok(id) if id == index as u32 => break,
                                Ok(_) => return Err(Error::ProtocolError),
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                                Err(_) => return Err(Error::Cancelled),
                            }
                        }
                        worker_root.check()?;
                        let (future_tx, future_rx) = mpsc::sync_channel(16);
                        let source = Source {
                            body: Body::Stream(future_rx),
                            root: worker_root.clone(),
                            max_samples: config.max_samples,
                            index: index as u32 + 1,
                            origin,
                            ready_at: None,
                            verified: if index + 1 < parts.len() {
                                Some(verified.clone())
                            } else {
                                None
                            },
                            claimed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                        };
                        send(&ready_tx, Ok(source), &worker_root)?;
                        worker_calls.fetch_add(1, Ordering::AcqRel);
                        let result = synth(
                            &config,
                            text,
                            worker_root.model_child(config.deadline_ms)?,
                            &mut |bytes| {
                                for part in bytes.chunks(9600) {
                                    send(
                                        &future_tx,
                                        Part::Data(Zeroizing::new(part.to_vec())),
                                        &worker_root,
                                    )?;
                                }
                                Ok(())
                            },
                        );
                        let failed = result.is_err();
                        send(&future_tx, Part::End(result), &worker_root)?;
                        if failed {
                            return Ok(());
                        }
                        loop {
                            worker_root.check()?;
                            match permit_rx.recv_timeout(Duration::from_millis(10)) {
                                Ok(()) => break,
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                                Err(_) => return Err(Error::Cancelled),
                            }
                        }
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    let _ = send(&ready_tx, Err(error), &worker_root);
                }
            })
            .map_err(|_| Error::IoFailed)?;
        Ok(Self {
            first: Some(first),
            ready,
            permit,
            root,
            worker: Some(worker),
            count,
            taken: 0,
            calls,
            origin,
        })
    }
    pub fn next_source(&mut self) -> Result<Option<Source>, Error> {
        self.next_source_with(|| Ok(()))
    }
    pub fn next_source_with(
        &mut self,
        mut idle: impl FnMut() -> Result<(), Error>,
    ) -> Result<Option<Source>, Error> {
        idle().inspect_err(|_| self.root.cancel())?;
        self.root.check()?;
        if self.taken == self.count {
            return Ok(None);
        }
        let source = if let Some(first) = self.first.take() {
            first
        } else {
            loop {
                self.root.check()?;
                idle().inspect_err(|_| self.root.cancel())?;
                match self.ready.recv_timeout(Duration::from_millis(10)) {
                    Ok(result) => break result?,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(_) => return Err(Error::OutcomeUnknown),
                }
            }
        };
        if source.index != self.taken + 1 {
            return Err(Error::ProtocolError);
        }
        if self.taken > 0 {
            self.permit.try_send(()).map_err(|_| Error::ProtocolError)?;
        }
        self.taken += 1;
        Ok(Some(source))
    }
    pub fn model_calls(&self) -> u64 {
        self.calls.load(Ordering::Acquire)
    }
}
impl Drop for Pipeline {
    fn drop(&mut self) {
        self.root.cancel();
        self.first.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Config {
        Config {
            endpoint: "http://127.0.0.1:50000/inference_zero_shot".into(),
            reference_file: "E:/unused.wav".into(),
            prompt_text: "<|endofprompt|>".into(),
            deadline_ms: 1000,
            max_samples: 96000,
            lookahead: Default::default(),
        }
    }
    fn summary() -> Summary {
        Summary {
            samples: 2,
            first_pcm_ms: 0,
            complete_ms: 0,
            transport_eof_ms: 0,
        }
    }
    fn fake(
        _: &Config,
        _: &str,
        op: Operation,
        push: &mut dyn FnMut(&[u8]) -> Result<(), Error>,
    ) -> Result<Summary, Error> {
        op.check()?;
        push(&[1, 0, 2, 0])?;
        Ok(summary())
    }
    fn wait_calls(p: &Pipeline, n: u64) {
        let began = Instant::now();
        while p.model_calls() < n {
            assert!(began.elapsed() < Duration::from_secs(1));
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn only_one_future_until_admission_and_single_model_thread() {
        let root = Operation::model_task(3000).unwrap();
        let mut p =
            Pipeline::start(config(), "第一句。第二句。第三句。", root.clone(), fake).unwrap();
        let first = p.next_source().unwrap().unwrap();
        let permit = first.prefetch_permit();
        let duplicate = first.prefetch_permit();
        assert_eq!(first.consume(root.clone(), |_| Ok(())).unwrap().samples, 2);
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(p.model_calls(), 1);
        permit.ready().unwrap();
        assert!(duplicate.ready().is_err());
        wait_calls(&p, 2);
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(p.model_calls(), 2);
        let second = p.next_source().unwrap().unwrap();
        assert!(second.ready_at.is_none()); // future GPU can consume before TTS EOS
        let permit = second.prefetch_permit();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(p.model_calls(), 2);
        second
            .consume(root.clone(), |b| {
                assert_eq!(b, [1, 0, 2, 0]);
                Ok(())
            })
            .unwrap();
        permit.ready().unwrap();
        wait_calls(&p, 3);
        let first = p.next_source().unwrap().unwrap();
        let permit = first.prefetch_permit();
        first.consume(root.clone(), |_| Ok(())).unwrap();
        permit.ready().unwrap();
        assert!(p.next_source().unwrap().is_none());
    }
    #[test]
    fn old_prepared_source_rejected_after_whole_script_cancel() {
        let root = Operation::model_task(3000).unwrap();
        let mut p = Pipeline::start(config(), "第一句。第二句。", root.clone(), fake).unwrap();
        let first = p.next_source().unwrap().unwrap();
        let permit = first.prefetch_permit();
        first.consume(root.clone(), |_| Ok(())).unwrap();
        permit.ready().unwrap();
        let old = p.next_source().unwrap().unwrap();
        root.cancel();
        assert!(
            old.consume(Operation::model_task(1000).unwrap(), |_| panic!("late PCM"))
                .is_err()
        );
        assert!(p.next_source().is_err());
    }
    #[test]
    fn first_source_delivers_pcm_before_synthesis_finishes() {
        let root = Operation::model_task(3000).unwrap();
        let released = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker = released.clone();
        let mut p = Pipeline::start(
            config(),
            "第一句。",
            root.clone(),
            move |_, _, op, push| {
                push(&[1, 0, 2, 0])?;
                while !worker.load(Ordering::Acquire) {
                    op.check()?;
                    std::thread::sleep(Duration::from_millis(1));
                }
                Ok(summary())
            },
        )
        .unwrap();
        p.next_source()
            .unwrap()
            .unwrap()
            .consume(root, |_| {
                assert!(!released.load(Ordering::Acquire));
                released.store(true, Ordering::Release);
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn disconnected_stream_is_not_verified_eos() {
        let (tx, rx) = mpsc::sync_channel(1);
        drop(tx);
        let source = Source {
            body: Body::Stream(rx),
            root: Operation::model_task(1000).unwrap(),
            max_samples: 10,
            index: 1,
            origin: Instant::now(),
            ready_at: None,
            verified: None,
            claimed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        assert!(
            source
                .consume(Operation::model_task(1000).unwrap(), |_| Ok(()))
                .is_err()
        );
    }
    #[test]
    fn cancelled_prefetch_permit_cannot_start_future_request() {
        let root = Operation::model_task(3000).unwrap();
        let mut p = Pipeline::start(config(), "第一句。第二句。", root.clone(), fake).unwrap();
        let first = p.next_source().unwrap().unwrap();
        let permit = first.prefetch_permit();
        first.consume(root.clone(), |_| Ok(())).unwrap();
        root.cancel();
        assert!(permit.ready().is_err());
        assert_eq!(p.model_calls(), 1);
    }
    #[test]
    fn waiting_source_pumps_close_callback_and_cancels_model_root() {
        let root = Operation::model_task(3000).unwrap();
        let mut p = Pipeline::start(config(), "第一句。第二句。", root.clone(), fake).unwrap();
        p.next_source()
            .unwrap()
            .unwrap()
            .consume(root.clone(), |_| Ok(()))
            .unwrap();
        let began = Instant::now();
        let mut polls = 0;
        let result = p.next_source_with(|| {
            polls += 1;
            if polls == 4 {
                Err(Error::Cancelled)
            } else {
                Ok(())
            }
        });
        assert!(matches!(result, Err(Error::Cancelled)));
        assert_eq!(polls, 4);
        assert!(began.elapsed() < Duration::from_millis(500));
        assert!(root.check().is_err());
        assert_eq!(p.model_calls(), 1);
    }
    #[test]
    fn future_source_delivers_before_its_provider_eos() {
        let root = Operation::model_task(3000).unwrap();
        let released = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = released.clone();
        let mut calls = 0;
        let mut p = Pipeline::start(
            config(),
            "第一句。第二句。",
            root.clone(),
            move |_, _, op, push| {
                calls += 1;
                push(&[1, 0, 2, 0])?;
                if calls == 2 {
                    while !flag.load(Ordering::Acquire) {
                        op.check()?;
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
                Ok(summary())
            },
        )
        .unwrap();
        let first = p.next_source().unwrap().unwrap();
        let permit = first.prefetch_permit();
        first.consume(root.clone(), |_| Ok(())).unwrap();
        permit.ready().unwrap();
        p.next_source()
            .unwrap()
            .unwrap()
            .consume(root, |_| {
                assert!(!released.load(Ordering::Acquire));
                released.store(true, Ordering::Release);
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn activation_credit_cannot_start_next_request_before_provider_eos() {
        let root = Operation::model_task(3000).unwrap();
        let eos = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let gate = eos.clone();
        let mut n = 0;
        let mut p = Pipeline::start(
            config(),
            "第一句。第二句。",
            root.clone(),
            move |_, _, op, push| {
                n += 1;
                push(&[1, 0, 2, 0])?;
                if n == 1 {
                    while !gate.load(Ordering::Acquire) {
                        op.check()?;
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
                Ok(summary())
            },
        )
        .unwrap();
        let first = p.next_source().unwrap().unwrap();
        first.prefetch_permit().ready().unwrap();
        wait_calls(&p, 1);
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(p.model_calls(), 1);
        eos.store(true, Ordering::Release);
        first.consume(root.clone(), |_| Ok(())).unwrap();
        p.next_source()
            .unwrap()
            .unwrap()
            .consume(root, |_| Ok(()))
            .unwrap();
        assert_eq!(p.model_calls(), 2);
    }
}
