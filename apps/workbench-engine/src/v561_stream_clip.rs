//! Incremental committed frame spool. Readiness is not generation completion.
use crate::{
    v561_script_source::{PrefetchPermit, Source},
    v561_stream_synthesis::{MediaSink, Production},
};
use nai_contracts::{v561_media::FrameRef, workbench::WorkbenchError as Error};
use nai_media_core::v561_frame_scheduler::Frame;
use nai_runtime_core::foundation::Operation;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};
use zeroize::Zeroizing;
#[derive(Clone, Copy)]
pub struct Facts {
    pub index: u32,
    pub origin: Instant,
    pub ready_at: Option<Instant>,
}
struct State {
    file: std::fs::File,
    pcm: Zeroizing<Vec<u8>>,
    frames: Vec<FrameRef>,
    sealed: Option<[u8; 32]>,
    production: Option<Production>,
    failure: Option<Error>,
    claimed: bool,
    eos_at: Option<Instant>,
    first_at: Option<Instant>,
    done_at: Option<Instant>,
    first_read_at: Option<Instant>,
    pcm_chunks: u64,
    first_pcm_at: Option<Instant>,
    last_pcm_at: Option<Instant>,
    render_stats: Option<nai_contracts::v561_avatar::RenderStats>,
}
#[derive(Clone)]
pub struct Observer {
    state: Arc<Mutex<State>>,
    origin: Instant,
    started: Instant,
}
impl Observer {
    pub fn metrics(&self) -> serde_json::Value {
        let Ok(s) = self.state.lock() else {
            return serde_json::Value::Null;
        };
        let ms = |at: Option<Instant>| {
            at.map(|at| at.saturating_duration_since(self.origin).as_millis())
        };
        let render = s.render_stats.as_ref();
        serde_json::json!({"scope":"root_observed_generation_progress","clock_origin":"script_monotonic",
            "started_ms":ms(Some(self.started)),"source_verified_ms":ms(s.eos_at),
            "first_frame_ms":ms(s.first_at),"first_consumed_ms":ms(s.first_read_at),"completed_ms":ms(s.done_at),
            "pcm_samples_observed":s.pcm.len()/2,"pcm_chunks_observed":s.pcm_chunks,
            "first_pcm_observed_ms":ms(s.first_pcm_at),"last_pcm_observed_ms":ms(s.last_pcm_at),
            "verified_frames":s.frames.len(),"source_eos_verified":s.sealed.is_some(),
            "render_completed":s.done_at.is_some(),"failure_code":s.failure.map(|e|e.as_str_name()),
            "pipeline_compute_wall_ms":render.map(|r|r.compute_ms),"transfer_ms":render.map(|r|r.transfer_ms),
            "delivery_ms":render.map(|r|r.delivery_ms),"compute_delivery_overlap_ms":render.map(|r|r.overlap_ms),
            "render_blocks":render.map(|r|r.blocks)})
    }
}
pub struct Clip {
    pub facts: Facts,
    pub epoch: u64,
    pub maximum: u64,
    state: Arc<Mutex<State>>,
    root: Operation,
    permit: PrefetchPermit,
    promoted: Arc<AtomicU32>,
    observer: Observer,
}
pub struct Writer {
    state: Arc<Mutex<State>>,
    maximum: u64,
    epoch: u64,
    pixel_bytes: u64,
}
impl Clip {
    pub(crate) fn channel(
        source: &Source,
        epoch: u64,
        directory: &Path,
        pixel_bytes: u64,
        root: Operation,
        promoted: Arc<AtomicU32>,
    ) -> Result<(Self, Writer), Error> {
        if !(1..=144000).contains(&source.max_samples)
            || epoch == 0
            || pixel_bytes == 0
            || pixel_bytes > 4194304
        {
            return Err(Error::InvalidArgument);
        }
        let started = Instant::now();
        let state = Arc::new(Mutex::new(State {
            file: tempfile::tempfile_in(directory).map_err(|_| Error::IoFailed)?,
            pcm: Zeroizing::new(Vec::new()),
            frames: Vec::new(),
            sealed: None,
            production: None,
            failure: None,
            claimed: false,
            eos_at: None,
            first_at: None,
            done_at: None,
            first_read_at: None,
            pcm_chunks: 0,
            first_pcm_at: None,
            last_pcm_at: None,
            render_stats: None,
        }));
        let observer = Observer {
            state: state.clone(),
            origin: source.origin,
            started,
        };
        Ok((
            Self {
                facts: Facts {
                    index: source.index,
                    origin: source.origin,
                    ready_at: None,
                },
                epoch,
                maximum: source.max_samples,
                state: state.clone(),
                root,
                permit: source.prefetch_permit(),
                promoted,
                observer,
            },
            Writer {
                state,
                maximum: source.max_samples,
                epoch,
                pixel_bytes,
            },
        ))
    }
    pub fn observer(&self) -> Observer {
        self.observer.clone()
    }
    pub fn replay(
        self,
        sink: &mut dyn MediaSink,
        op: &Operation,
        mut capacity: impl FnMut() -> Result<(), Error>,
    ) -> Result<Production, Error> {
        self.root.check()?;
        op.check()?;
        {
            let mut s = self.state.lock().map_err(|_| Error::IoFailed)?;
            if s.claimed {
                return Err(Error::ProtocolError);
            }
            s.claimed = true;
        }
        self.promoted.store(self.facts.index, Ordering::Release);
        // Resource admission only. Producer is serial and cannot issue the next
        // model request until the current provider has returned a valid EOS.
        self.permit.ready()?;
        let mut bytes = 0usize;
        let mut frame = 0usize;
        let mut eos = false;
        loop {
            self.root.check()?;
            op.check()?;
            let mut progress = false;
            let pcm = {
                let s = self.state.lock().map_err(|_| Error::IoFailed)?;
                if let Some(e) = s.failure {
                    return Err(e);
                }
                if bytes < s.pcm.len() {
                    Some(Zeroizing::new(
                        s.pcm[bytes..(bytes + 9600).min(s.pcm.len())].to_vec(),
                    ))
                } else {
                    None
                }
            };
            if let Some(pcm) = pcm {
                sink.pcm(&pcm)?;
                bytes += pcm.len();
                progress = true;
            }
            let sealed = {
                let s = self.state.lock().map_err(|_| Error::IoFailed)?;
                s.sealed
                    .filter(|_| !eos && bytes == s.pcm.len())
                    .map(|hash| (bytes as u64 / 2, hash))
            };
            if let Some((samples, hash)) = sealed {
                sink.verified_eos(samples, &hash)?;
                eos = true;
                progress = true;
            }
            let available = {
                self.state
                    .lock()
                    .map_err(|_| Error::IoFailed)?
                    .frames
                    .get(frame)
                    .cloned()
            };
            if let Some(metadata) = available {
                // Frames may not overtake the PCM snapshot delivered to its gate.
                let at = metadata.source_sample.ok_or(Error::ProtocolError)?;
                if at < bytes as u64 / 2 && (eos || at + 960 <= bytes as u64 / 2) {
                    capacity()?;
                    self.root.check()?;
                    op.check()?;
                    let mut pixels = vec![0; metadata.payload_bytes as usize];
                    {
                        let mut s = self.state.lock().map_err(|_| Error::IoFailed)?;
                        let offset = (frame as u64)
                            .checked_mul(metadata.payload_bytes)
                            .ok_or(Error::ProtocolError)?;
                        s.file
                            .seek(SeekFrom::Start(offset))
                            .map_err(|_| Error::IoFailed)?;
                        s.file
                            .read_exact(&mut pixels)
                            .map_err(|_| Error::IoFailed)?;
                    }
                    if metadata.epoch != self.epoch
                        || Sha256::digest(&pixels).as_slice() != metadata.content_sha256
                    {
                        return Err(Error::ProtocolError);
                    }
                    sink.frame(Frame { metadata, pixels })?;
                    self.state
                        .lock()
                        .map_err(|_| Error::IoFailed)?
                        .first_read_at
                        .get_or_insert_with(Instant::now);
                    frame += 1;
                    progress = true;
                }
            }
            {
                let mut s = self.state.lock().map_err(|_| Error::IoFailed)?;
                if let Some(e) = s.failure {
                    return Err(e);
                }
                if s.done_at.is_some() && frame == s.frames.len() && eos && bytes == s.pcm.len() {
                    let p = s.production.take().ok_or(Error::ProtocolError)?;
                    if p.provider.samples != bytes as u64 / 2
                        || p.render.sha256 != Sha256::digest(s.pcm.as_slice()).as_slice()
                    {
                        return Err(Error::ProtocolError);
                    }
                    return Ok(p);
                }
            }
            if !progress {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}
impl Writer {
    pub fn finish(&mut self, result: Result<Production, Error>) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::IoFailed)?;
        match result {
            Ok(p) => {
                let length = match s.file.metadata() {
                    Ok(m) => m.len(),
                    Err(_) => {
                        s.failure = Some(Error::IoFailed);
                        return Err(Error::IoFailed);
                    }
                };
                let good = s.sealed.is_some()
                    && s.frames.len() as u64 == p.provider.samples.div_ceil(960)
                    && p.provider.samples == s.pcm.len() as u64 / 2
                    && p.render.sha256 == Sha256::digest(s.pcm.as_slice()).as_slice()
                    && length == s.frames.len() as u64 * self.pixel_bytes;
                if !good {
                    s.failure = Some(Error::ProtocolError);
                    return Err(Error::ProtocolError);
                }
                s.render_stats = p.render.stats;
                s.production = Some(p);
                s.done_at = Some(Instant::now());
                Ok(())
            }
            Err(e) => {
                s.failure = Some(e);
                Err(e)
            }
        }
    }
}
impl MediaSink for Writer {
    fn pcm(&mut self, pcm: &[u8]) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::IoFailed)?;
        if s.sealed.is_some()
            || pcm.is_empty()
            || !pcm.len().is_multiple_of(2)
            || pcm.len() as u64 / 2 > self.maximum - s.pcm.len() as u64 / 2
        {
            return Err(Error::ProtocolError);
        }
        s.pcm.extend_from_slice(pcm);
        let now = Instant::now();
        s.pcm_chunks += 1;
        s.first_pcm_at.get_or_insert(now);
        s.last_pcm_at = Some(now);
        Ok(())
    }
    fn frame(&mut self, frame: Frame) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::IoFailed)?;
        let n = s.frames.len() as u64;
        if n >= self.maximum.div_ceil(960)
            || frame.metadata.epoch != self.epoch
            || frame.metadata.sequence != Some(n)
            || frame.metadata.source_sample != Some(n * 960)
            || n * 960 >= s.pcm.len() as u64 / 2
            || (s.sealed.is_none() && n * 960 + 960 > s.pcm.len() as u64 / 2)
            || frame.metadata.payload_bytes != self.pixel_bytes
            || frame.pixels.len() as u64 != self.pixel_bytes
            || frame.metadata.content_sha256 != Sha256::digest(&frame.pixels).as_slice()
            || s.frames
                .first()
                .is_some_and(|f| f.pool_id != frame.metadata.pool_id)
        {
            return Err(Error::ProtocolError);
        }
        s.file.seek(SeekFrom::End(0)).map_err(|_| Error::IoFailed)?;
        s.file
            .write_all(&frame.pixels)
            .map_err(|_| Error::IoFailed)?;
        s.frames.push(frame.metadata);
        s.first_at.get_or_insert_with(Instant::now);
        Ok(())
    }
    fn verified_eos(&mut self, samples: u64, hash: &[u8]) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::IoFailed)?;
        if s.sealed.is_some()
            || samples == 0
            || samples != s.pcm.len() as u64 / 2
            || hash != Sha256::digest(s.pcm.as_slice()).as_slice()
        {
            return Err(Error::ProtocolError);
        }
        s.sealed = Some(hash.try_into().map_err(|_| Error::ProtocolError)?);
        s.eos_at = Some(Instant::now());
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_generation_survives_failure_without_fabricated_completion_times() {
        let dir = tempfile::tempdir().unwrap();
        let op = Operation::model_task(1000).unwrap();
        let source = Source::test_empty(op.clone(), 960);
        let (clip, mut writer) =
            Clip::channel(&source, 1, dir.path(), 4, op, Arc::new(AtomicU32::new(0))).unwrap();
        let observer = clip.observer();
        writer.pcm(&vec![1; 1920]).unwrap();
        writer.frame(frame(1)).unwrap();
        assert_eq!(writer.finish(Err(Error::Cancelled)), Err(Error::Cancelled));
        let metrics = observer.metrics();
        assert_eq!(metrics["pcm_samples_observed"], 960);
        assert_eq!(metrics["pcm_chunks_observed"], 1);
        assert_eq!(metrics["verified_frames"], 1);
        assert_eq!(metrics["source_eos_verified"], false);
        assert_eq!(metrics["render_completed"], false);
        assert!(metrics["first_pcm_observed_ms"].is_number());
        assert!(metrics["completed_ms"].is_null());
        assert!(metrics["pipeline_compute_wall_ms"].is_null());
        assert_eq!(metrics["failure_code"], Error::Cancelled.as_str_name());
    }
    fn frame(epoch: u64) -> Frame {
        let pixels = vec![1, 2, 3, 4];
        Frame {
            metadata: FrameRef {
                pool_id: "fixture".into(),
                epoch,
                sequence: Some(0),
                source_sample: Some(0),
                payload_bytes: 4,
                content_sha256: Sha256::digest(&pixels).to_vec(),
                ..Default::default()
            },
            pixels,
        }
    }
    fn production() -> Production {
        Production {
            provider: nai_llm_openai::cosyvoice::Summary {
                samples: 960,
                first_pcm_ms: 0,
                complete_ms: 0,
                transport_eof_ms: 0,
            },
            render: nai_contracts::v561_avatar_stream::Event {
                sha256: Sha256::digest(vec![1; 1920]).to_vec(),
                stats: Some(nai_contracts::v561_avatar::RenderStats {
                    compute_ms: 11,
                    transfer_ms: 3,
                    delivery_ms: 5,
                    overlap_ms: 2,
                    blocks: 1,
                    ..Default::default()
                }),
                ..Default::default()
            },
            first_pcm_ms: 0,
            provider_verified_ms: 0,
            first_frame_ms: Some(0),
            completed_ms: 0,
        }
    }
    struct Consumer {
        frames: u32,
        ack: Option<std::sync::mpsc::SyncSender<()>>,
    }
    impl MediaSink for Consumer {
        fn pcm(&mut self, _: &[u8]) -> Result<(), Error> {
            Ok(())
        }
        fn verified_eos(&mut self, _: u64, _: &[u8]) -> Result<(), Error> {
            Ok(())
        }
        fn frame(&mut self, _: Frame) -> Result<(), Error> {
            self.frames += 1;
            if let Some(tx) = self.ack.take() {
                tx.send(()).unwrap();
            }
            Ok(())
        }
    }
    #[test]
    fn committed_frame_is_consumed_before_eos_and_render_completion() {
        let dir = tempfile::tempdir().unwrap();
        let op = Operation::model_task(2000).unwrap();
        let source = Source::test_empty(op.clone(), 960);
        let (clip, mut writer) = Clip::channel(
            &source,
            1,
            dir.path(),
            4,
            op.clone(),
            Arc::new(AtomicU32::new(0)),
        )
        .unwrap();
        let observer = clip.observer();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let t = std::thread::spawn(move || {
            writer.pcm(&vec![1; 1920]).unwrap();
            writer.frame(frame(1)).unwrap();
            rx.recv_timeout(Duration::from_secs(1)).unwrap(); // reader must run before producer finishes
            writer
                .verified_eos(960, &Sha256::digest(vec![1; 1920]))
                .unwrap();
            writer.finish(Ok(production())).unwrap();
        });
        let mut consumer = Consumer {
            frames: 0,
            ack: Some(tx),
        };
        clip.replay(&mut consumer, &op, || Ok(())).unwrap();
        t.join().unwrap();
        assert_eq!(consumer.frames, 1);
        assert_eq!(observer.metrics()["pipeline_compute_wall_ms"], 11);
        assert_eq!(observer.metrics()["render_completed"], true);
        drop(observer);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
    #[test]
    fn corrupt_frame_and_cancelled_reader_do_not_deliver_pixels() {
        let dir = tempfile::tempdir().unwrap();
        let op = Operation::model_task(1000).unwrap();
        let source = Source::test_empty(op.clone(), 960);
        let (clip, mut writer) = Clip::channel(
            &source,
            1,
            dir.path(),
            4,
            op.clone(),
            Arc::new(AtomicU32::new(0)),
        )
        .unwrap();
        writer.pcm(&vec![1; 1920]).unwrap();
        writer.frame(frame(1)).unwrap();
        {
            let mut s = writer.state.lock().unwrap();
            s.file.seek(SeekFrom::Start(0)).unwrap();
            s.file.write_all(&[9; 4]).unwrap();
        }
        let mut consumer = Consumer {
            frames: 0,
            ack: None,
        };
        assert!(clip.replay(&mut consumer, &op, || Ok(())).is_err());
        assert_eq!(consumer.frames, 0);
        drop(writer);
        let (clip, writer) = Clip::channel(
            &source,
            1,
            dir.path(),
            4,
            op.clone(),
            Arc::new(AtomicU32::new(0)),
        )
        .unwrap();
        op.cancel();
        assert!(
            clip.replay(&mut consumer, &Operation::model_task(1000).unwrap(), || Ok(
                ()
            ))
            .is_err()
        );
        drop(writer);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
    #[test]
    fn file_and_identity_caps_and_failure_are_terminal_for_reader() {
        let dir = tempfile::tempdir().unwrap();
        let op = Operation::model_task(1000).unwrap();
        let source = Source::test_empty(op.clone(), 960);
        let (clip, mut writer) = Clip::channel(
            &source,
            1,
            dir.path(),
            4,
            op.clone(),
            Arc::new(AtomicU32::new(0)),
        )
        .unwrap();
        writer.pcm(&vec![1; 1920]).unwrap();
        assert!(writer.frame(frame(2)).is_err());
        writer.frame(frame(1)).unwrap();
        assert!(writer.frame(frame(1)).is_err());
        assert!(writer.finish(Err(Error::ProtocolError)).is_err());
        assert!(
            clip.replay(
                &mut Consumer {
                    frames: 0,
                    ack: None
                },
                &op,
                || Ok(())
            )
            .is_err()
        );
    }
}
