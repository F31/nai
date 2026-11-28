//! One synchronous PCM assembly slot. Blocks are provisional computation input,
//! never a playback grant. The current avatar worker does not consume these yet.
use nai_contracts::workbench::WorkbenchError as Error;
use nai_llm_openai::cosyvoice::{self, Config, Summary};
use nai_runtime_core::foundation::Operation;

pub struct Block {
    pub epoch: u64,
    pub task: u64,
    pub start_sample: u64,
    pub pcm: Vec<u8>,
    pub eos_verified: bool,
}
struct Input {
    epoch: u64,
    task: u64,
    block_bytes: usize,
    limit: u64,
    total: u64,
    delivered: u64,
    pending: Vec<u8>,
    operation: Operation,
}
impl Input {
    fn new(
        epoch: u64,
        task: u64,
        frames: u32,
        limit: u64,
        operation: Operation,
    ) -> Result<Self, Error> {
        operation.check()?;
        if epoch == 0 || task == 0 || !(1..=97).contains(&frames) || !(1..=144000).contains(&limit)
        {
            return Err(Error::InvalidArgument);
        }
        Ok(Self {
            epoch,
            task,
            block_bytes: frames as usize * 960 * 2,
            limit,
            total: 0,
            delivered: 0,
            pending: Vec::new(),
            operation,
        })
    }
    fn deliver(
        &mut self,
        verified: bool,
        emit: &mut impl FnMut(Block) -> Result<(), Error>,
    ) -> Result<(), Error> {
        self.operation.check()?;
        let pcm = std::mem::take(&mut self.pending);
        let count = (pcm.len() / 2) as u64;
        emit(Block {
            epoch: self.epoch,
            task: self.task,
            start_sample: self.delivered,
            pcm,
            eos_verified: verified,
        })?;
        self.operation.check()?;
        self.delivered += count;
        Ok(())
    }
    fn push(
        &mut self,
        mut bytes: &[u8],
        emit: &mut impl FnMut(Block) -> Result<(), Error>,
    ) -> Result<(), Error> {
        self.operation.check()?;
        if bytes.is_empty() || !bytes.len().is_multiple_of(2) {
            return Err(Error::ProtocolError);
        }
        let total = self
            .total
            .checked_add((bytes.len() / 2) as u64)
            .ok_or(Error::ResourceExhausted)?;
        if total > self.limit {
            return Err(Error::ResourceExhausted);
        }
        self.total = total;
        while !bytes.is_empty() {
            self.operation.check()?;
            let take = bytes.len().min(self.block_bytes - self.pending.len());
            self.pending.extend_from_slice(&bytes[..take]);
            bytes = &bytes[take..];
            if self.pending.len() == self.block_bytes {
                self.deliver(false, emit)?;
            }
        }
        Ok(())
    }
    fn finish(
        &mut self,
        samples: u64,
        emit: &mut impl FnMut(Block) -> Result<(), Error>,
    ) -> Result<(), Error> {
        self.operation.check()?;
        if self.total == 0 || samples != self.total {
            return Err(Error::ProtocolError);
        }
        if !self.pending.is_empty() {
            self.deliver(true, emit)?;
        }
        Ok(())
    }
}
/// Caller supplies geometry from verified Ready. On any error all previously
/// emitted provisional work is revoked through this same parent operation.
pub fn produce(
    config: &Config,
    text: &str,
    epoch: u64,
    task: u64,
    block_frames: u32,
    operation: Operation,
    mut emit: impl FnMut(Block) -> Result<(), Error>,
) -> Result<Summary, Error> {
    let result = (|| {
        let mut input = Input::new(
            epoch,
            task,
            block_frames,
            config.max_samples,
            operation.clone(),
        )?;
        let summary = cosyvoice::stream(config, text, operation.clone(), |part| {
            input.push(part, &mut emit)
        })?;
        input.finish(summary.samples, &mut emit)?;
        operation.check()?;
        Ok(summary)
    })();
    if result.is_err() {
        operation.cancel();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Input {
        Input::new(7, 8, 1, 4000, Operation::model_task(4000).unwrap()).unwrap()
    }
    #[test]
    fn partitioning_preserves_bytes_offsets_and_verified_tail() {
        let source = (0..5000).map(|i| i as u8).collect::<Vec<_>>();
        for stride in [2, 158, 1920, 5000] {
            let mut input = input();
            let mut blocks = Vec::new();
            for part in source.chunks(stride) {
                input
                    .push(part, &mut |b| {
                        blocks.push(b);
                        Ok(())
                    })
                    .unwrap();
            }
            assert_eq!(blocks.len(), 2); // full blocks arrive before EOS
            assert!(input.pending.len() < input.block_bytes);
            input
                .finish(2500, &mut |b| {
                    blocks.push(b);
                    Ok(())
                })
                .unwrap();
            assert_eq!(
                blocks.iter().map(|b| b.start_sample).collect::<Vec<_>>(),
                vec![0, 960, 1920]
            );
            assert_eq!(
                blocks
                    .iter()
                    .flat_map(|b| b.pcm.clone())
                    .collect::<Vec<_>>(),
                source
            );
            assert!(blocks.iter().all(|b| b.epoch == 7 && b.task == 8));
            assert!(!blocks[0].eos_verified && blocks[2].eos_verified);
        }
    }
    #[test]
    fn invalid_eos_never_delivers_tail() {
        let mut i = input();
        let mut emit = |_| panic!("tail must not escape");
        i.push(&[1, 0], &mut emit).unwrap();
        assert_eq!(i.finish(2, &mut emit), Err(Error::ProtocolError));
        i.operation.cancel();
        assert_eq!(i.finish(1, &mut emit), Err(Error::Cancelled));
    }
    #[test]
    fn consumer_failure_stops_remaining_blocks_and_overflow_is_atomic() {
        let mut i = input();
        let mut calls = 0;
        assert_eq!(
            i.push(&vec![0; 6000], &mut |_| {
                calls += 1;
                Err(Error::IoFailed)
            }),
            Err(Error::IoFailed)
        );
        assert_eq!(calls, 1);
        let mut i = input();
        assert_eq!(
            i.push(&vec![0; 8002], &mut |_| panic!("overflow")),
            Err(Error::ResourceExhausted)
        );
        assert!(i.pending.is_empty());
        assert_eq!(i.push(&[1], &mut |_| Ok(())), Err(Error::ProtocolError));
    }
}
