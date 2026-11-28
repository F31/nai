//! Bounded in-process source, not a transport or device authorization.
use nai_contracts::workbench::WorkbenchError as Error;
use nai_runtime_core::foundation::Operation;
use sha2::{Digest, Sha256};
use std::{sync::mpsc, time::Duration};
enum Message {
    Data(Vec<u8>),
    End(u64, Vec<u8>),
}
pub struct Sender {
    tx: mpsc::SyncSender<Message>,
    operation: Operation,
    maximum: u64,
    samples: u64,
    hash: Sha256,
    sealed: bool,
}
pub struct Source {
    rx: mpsc::Receiver<Message>,
    operation: Operation,
    pub(crate) maximum: u64,
}
pub fn channel(
    maximum: u64,
    capacity: usize,
    operation: Operation,
) -> Result<(Sender, Source), Error> {
    if maximum == 0 || maximum > 144000 || !(1..=16).contains(&capacity) {
        return Err(Error::InvalidArgument);
    }
    let (tx, rx) = mpsc::sync_channel(capacity);
    Ok((
        Sender {
            tx,
            operation: operation.clone(),
            maximum,
            samples: 0,
            hash: Sha256::new(),
            sealed: false,
        },
        Source {
            rx,
            operation,
            maximum,
        },
    ))
}
impl Sender {
    fn send(&self, message: Message) -> Result<(), Error> {
        self.operation.check()?;
        self.tx.try_send(message).map_err(|error| {
            self.operation.cancel();
            match error {
                mpsc::TrySendError::Full(_) => Error::ResourceExhausted,
                _ => Error::Cancelled,
            }
        })
    }
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if self.sealed
            || bytes.is_empty()
            || bytes.len() > 9600
            || !bytes.len().is_multiple_of(2)
            || bytes.len() as u64 / 2 > self.maximum - self.samples
        {
            self.operation.cancel();
            return Err(Error::InvalidArgument);
        }
        self.send(Message::Data(bytes.to_vec()))?;
        self.hash.update(bytes);
        self.samples += bytes.len() as u64 / 2;
        Ok(())
    }
    pub fn finish(&mut self) -> Result<(), Error> {
        if self.sealed || self.samples == 0 {
            self.operation.cancel();
            return Err(Error::ProtocolError);
        }
        self.send(Message::End(
            self.samples,
            self.hash.clone().finalize().to_vec(),
        ))?;
        self.sealed = true;
        Ok(())
    }
}
impl Source {
    pub(crate) fn consume(
        self,
        mut push: impl FnMut(&[u8]) -> Result<(), Error>,
    ) -> Result<u64, Error> {
        let result = (|| {
            let mut samples = 0;
            let mut hash = Sha256::new();
            loop {
                self.operation.check()?;
                match self.rx.recv_timeout(Duration::from_millis(10)) {
                    Ok(Message::Data(mut bytes)) => {
                        if bytes.is_empty()
                            || bytes.len() > 9600
                            || !bytes.len().is_multiple_of(2)
                            || bytes.len() as u64 / 2 > self.maximum - samples
                        {
                            return Err(Error::ProtocolError);
                        }
                        hash.update(&bytes);
                        samples += bytes.len() as u64 / 2;
                        let result = push(&bytes);
                        zeroize::Zeroize::zeroize(&mut bytes);
                        result?;
                    }
                    Ok(Message::End(count, digest)) => {
                        if samples == 0 || count != samples || digest != hash.finalize().as_slice()
                        {
                            return Err(Error::ProtocolError);
                        }
                        return Ok(samples);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(_) => return Err(Error::OutcomeUnknown),
                }
            }
        })();
        if result.is_err() {
            self.operation.cancel();
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn op() -> Operation {
        Operation::model_task(1000).unwrap()
    }
    #[test]
    fn bytes_and_verified_end_preserved() {
        let (mut tx, rx) = channel(100, 2, op()).unwrap();
        tx.push(&[1, 2, 3, 4]).unwrap();
        tx.finish().unwrap();
        let mut bytes = Vec::new();
        assert_eq!(
            rx.consume(|b| {
                bytes.extend_from_slice(b);
                Ok(())
            }),
            Ok(2)
        );
        assert_eq!(bytes, [1, 2, 3, 4]);
    }
    #[test]
    fn backpressure_is_nonblocking_and_revokes_parent() {
        let parent = op();
        let (mut tx, _rx) = channel(100, 1, parent.clone()).unwrap();
        tx.push(&[0, 0]).unwrap();
        assert_eq!(tx.push(&[0, 0]), Err(Error::ResourceExhausted));
        assert!(parent.check().is_err());
    }
    #[test]
    fn disconnected_and_bad_digest_never_complete() {
        for bad in [false, true] {
            let parent = op();
            let (mut tx, rx) = channel(100, 2, parent.clone()).unwrap();
            tx.push(&[0, 0]).unwrap();
            if bad {
                tx.tx.try_send(Message::End(1, vec![0; 32])).unwrap();
            }
            drop(tx);
            assert!(rx.consume(|_| Ok(())).is_err());
            assert!(parent.check().is_err());
        }
    }
    #[test]
    fn waiting_source_observes_cancel() {
        let parent = op();
        let (_tx, rx) = channel(100, 1, parent.clone()).unwrap();
        let thread = std::thread::spawn(move || rx.consume(|_| Ok(())));
        parent.cancel();
        assert_eq!(thread.join().unwrap(), Err(Error::Cancelled));
    }
}
