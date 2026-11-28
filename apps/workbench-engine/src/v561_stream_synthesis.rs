//! One source producer, one bounded delivery slot; owner keeps pumping frames.
use crate::v561_avatar_stream::Resident;
use nai_contracts::{v561_avatar_stream::Event, workbench::WorkbenchError as Error};
use nai_llm_openai::cosyvoice::{self, Config, Summary};
use nai_media_core::v561_frame_scheduler::Frame;
use nai_runtime_core::foundation::Operation;
use sha2::{Digest, Sha256};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

pub struct Production {
    pub provider: Summary,
    pub render: Event,
    pub first_pcm_ms: u128,
    pub provider_verified_ms: u128,
    pub first_frame_ms: Option<u128>,
    pub completed_ms: u128,
}
/// Owner-thread callbacks must be bounded and nonblocking. PCM and frame
/// delivery are source facts, not permission or evidence of device playback.
pub trait MediaSink {
    fn pcm(&mut self, bytes: &[u8]) -> Result<(), Error>;
    fn frame(&mut self, frame: Frame) -> Result<(), Error>;
    fn verified_frame(
        &mut self,
        frame: nai_media_core::v561_frame_scheduler::OwnedVerifiedFrame,
    ) -> Result<(), Error> {
        self.frame(frame.into_frame())
    }
    fn verified_eos(&mut self, samples: u64, digest: &[u8]) -> Result<(), Error>;
}
struct FramesOnly<F>(F);
impl<F: FnMut(Frame) -> Result<(), Error>> MediaSink for FramesOnly<F> {
    fn pcm(&mut self, _: &[u8]) -> Result<(), Error> {
        Ok(())
    }
    fn frame(&mut self, frame: Frame) -> Result<(), Error> {
        (self.0)(frame)
    }
    fn verified_eos(&mut self, _: u64, _: &[u8]) -> Result<(), Error> {
        Ok(())
    }
}
enum Input {
    Pcm(Vec<u8>, u128),
    Done(Result<Summary, Error>, u128),
}
fn send(tx: &mpsc::SyncSender<Input>, mut value: Input, op: &Operation) -> Result<(), Error> {
    loop {
        op.check()?;
        match tx.try_send(value) {
            Ok(()) => return Ok(()),
            Err(mpsc::TrySendError::Full(v)) => {
                value = v;
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(mpsc::TrySendError::Disconnected(_)) => return Err(Error::Cancelled),
        }
    }
}

pub fn generate(
    owner: &mut Resident,
    config: &Config,
    text: &str,
    task: u64,
    epoch: u64,
    operation: Operation,
    accept: impl FnMut(Frame) -> Result<(), Error>,
) -> Result<Production, Error> {
    generate_media(
        owner,
        config,
        text,
        task,
        epoch,
        operation,
        &mut FramesOnly(accept),
    )
}

pub fn generate_media(
    owner: &mut Resident,
    config: &Config,
    text: &str,
    task: u64,
    epoch: u64,
    operation: Operation,
    sink: &mut dyn MediaSink,
) -> Result<Production, Error> {
    generate_source(
        owner,
        config.max_samples,
        task,
        epoch,
        operation,
        sink,
        move |op, accept| {
            cosyvoice::stream(config, text, op.model_child(config.deadline_ms)?, accept)
        },
    )
}
pub fn generate_source(
    owner: &mut Resident,
    max_samples: u64,
    task: u64,
    epoch: u64,
    operation: Operation,
    sink: &mut dyn MediaSink,
    produce: impl FnOnce(
        Operation,
        &mut dyn FnMut(&[u8]) -> Result<(), Error>,
    ) -> Result<Summary, Error>
    + Send,
) -> Result<Production, Error> {
    if !(1..=144000).contains(&max_samples) {
        return Err(Error::InvalidArgument);
    }
    operation.check()?;
    let began = Instant::now();
    owner.begin(task, epoch, max_samples, &operation)?;
    std::thread::scope(|scope| {
        let (tx, rx) = mpsc::sync_channel(1);
        let producer_op = operation.clone();
        let producer = std::thread::Builder::new()
            .name("tts-source".into())
            .spawn_scoped(scope, move || {
                let result = produce(producer_op.clone(), &mut |bytes| {
                    for part in bytes.chunks(9600) {
                        send(
                            &tx,
                            Input::Pcm(part.to_vec(), began.elapsed().as_millis()),
                            &producer_op,
                        )?;
                    }
                    Ok(())
                });
                let verified_at = began.elapsed().as_millis();
                let _ = send(&tx, Input::Done(result, verified_at), &producer_op);
            });
        let producer = match producer {
            Ok(thread) => thread,
            Err(_) => {
                operation.cancel();
                let _ = owner.cancel();
                return Err(Error::IoFailed);
            }
        };
        let sink = std::cell::RefCell::new(sink);
        let mut first_frame = None;
        let mut frame = |f| {
            first_frame.get_or_insert(began.elapsed().as_millis());
            sink.borrow_mut().frame(f)
        };
        let result = (|| {
            let mut first_pcm = None;
            let mut hash = Sha256::new();
            let mut samples = 0;
            loop {
                operation.check()?;
                owner.poll_frames(&mut frame)?;
                match rx.recv_timeout(Duration::from_millis(10)) {
                    Ok(Input::Pcm(bytes, received)) => {
                        first_pcm.get_or_insert(received);
                        sink.borrow_mut().pcm(&bytes)?;
                        operation.check()?;
                        owner.chunk(&bytes, &mut frame)?;
                        hash.update(&bytes);
                        samples += (bytes.len() / 2) as u64;
                    }
                    Ok(Input::Done(result, verified_at)) => {
                        let provider = result?;
                        operation.check()?;
                        if provider.samples != samples {
                            return Err(Error::ProtocolError);
                        }
                        let digest = hash.finalize();
                        sink.borrow_mut().verified_eos(samples, &digest)?;
                        operation.check()?;
                        let render = owner.finish_verified(samples, &digest, &mut frame)?;
                        return Ok((
                            provider,
                            render,
                            first_pcm.ok_or(Error::ProtocolError)?,
                            verified_at,
                        ));
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(_) => return Err(Error::OutcomeUnknown),
                }
            }
        })();
        if result.is_err() {
            operation.cancel();
            let _ = owner.cancel();
        }
        drop(rx);
        if producer.join().is_err() {
            operation.cancel();
            let _ = owner.cancel();
            return Err(Error::OutcomeUnknown);
        }
        let (provider, render, first_pcm_ms, provider_verified_ms) = result?;
        Ok(Production {
            provider,
            render,
            first_pcm_ms,
            provider_verified_ms,
            first_frame_ms: first_frame,
            completed_ms: began.elapsed().as_millis(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_single_slot_observes_cancel_without_enqueuing_a_second_block() {
        let (tx, rx) = mpsc::sync_channel(1);
        let op = Operation::new(1000).unwrap();
        tx.send(Input::Pcm(vec![1; 2], 0)).unwrap();
        let worker_op = op.clone();
        let thread = std::thread::spawn(move || send(&tx, Input::Pcm(vec![2; 2], 1), &worker_op));
        std::thread::sleep(Duration::from_millis(20));
        let start = Instant::now();
        op.cancel();
        assert!(matches!(thread.join().unwrap(), Err(Error::Cancelled)));
        assert!(start.elapsed() < Duration::from_millis(500));
        match rx.try_recv().unwrap() {
            Input::Pcm(bytes, _) => assert_eq!(bytes, vec![1; 2]),
            _ => panic!("unexpected terminal"),
        };
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn disconnected_consumer_does_not_retry_delivery() {
        let (tx, rx) = mpsc::sync_channel(1);
        drop(rx);
        assert!(matches!(
            send(
                &tx,
                Input::Pcm(vec![0; 2], 0),
                &Operation::new(1000).unwrap()
            ),
            Err(Error::Cancelled)
        ));
    }
}
