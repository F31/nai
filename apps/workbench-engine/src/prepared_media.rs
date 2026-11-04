//! Root-owned anonymous PCM data plane. No pathname, media bytes or write handle reaches the page.
use nai_contracts::workbench::{
    PreparedMediaDescriptor, PreparedMediaReceipt, PublicMediaVisual, WorkbenchError as Error,
};
use nai_media_core::{
    public_avatar::BuiltinAvatar,
    public_clip::{FRAME_SAMPLES, PublicClipRenderer, RENDERER_ID, expected_frames},
};
use nai_runtime_core::foundation::{Operation, StopFence, uuid};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Instant,
};
use zeroize::Zeroizing;
struct WorkerPermit(Arc<AtomicUsize>);
impl Drop for WorkerPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
type Result<T> = std::result::Result<T, Error>;
struct Entry {
    ws: String,
    id: String,
    sha: String,
    file: File,
    view: PreparedMediaDescriptor,
    receipt: Option<PreparedMediaReceipt>,
    until: Instant,
    generation: u64,
}
struct Job {
    ws: String,
    id: String,
    until: Instant,
    op: Operation,
    rx: mpsc::Receiver<Result<Entry>>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.op.cancel();
    }
}
pub struct Bank {
    root: PathBuf,
    fence: Arc<StopFence>,
    items: Vec<Entry>,
    jobs: Vec<Job>,
    active: Arc<AtomicUsize>,
}
pub struct Source<'a> {
    pub workspace: &'a str,
    pub task: &'a str,
    pub task_sha: &'a str,
    pub samples: super::tts_audio::PreparationSamples<'a>,
}
/// Opaque, bounded read view. No path, file descriptor or Write implementation is exported.
pub struct PcmReader {
    samples: Zeroizing<Vec<i16>>,
}
impl PcmReader {
    pub fn samples(&self) -> &[i16] {
        &self.samples
    }
}
fn read(
    file: &File,
    view: &PreparedMediaDescriptor,
    check: &dyn Fn() -> Result<()>,
) -> Result<PcmReader> {
    check()?;
    let a = view.audio.as_ref().ok_or(Error::IntegrityFailed)?;
    if a.sample_count == 0
        || a.sample_count > 480000
        || a.sample_rate != 24000
        || a.channels != 1
        || file.metadata().map_err(|_| Error::IoFailed)?.len() != a.sample_count * 2
    {
        return Err(Error::IntegrityFailed);
    }
    let mut file = file.try_clone().map_err(|_| Error::IoFailed)?;
    file.seek(SeekFrom::Start(0)).map_err(|_| Error::IoFailed)?;
    let mut bytes = Zeroizing::new(Vec::new());
    bytes
        .try_reserve_exact((a.sample_count * 2) as usize)
        .map_err(|_| Error::ResourceExhausted)?;
    let mut buf = Zeroizing::new([0u8; 8192]);
    let mut h = Sha256::new();
    loop {
        check()?;
        let n = file.read(&mut *buf).map_err(|_| Error::IoFailed)?;
        if n == 0 {
            break;
        }
        if bytes.len() + n > a.sample_count as usize * 2 {
            return Err(Error::IntegrityFailed);
        }
        h.update(&buf[..n]);
        bytes.extend_from_slice(&buf[..n]);
    }
    let sha: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if sha != a.pcm_sha256 || bytes.len() as u64 != a.sample_count * 2 {
        return Err(Error::IntegrityFailed);
    }
    let mut samples = Zeroizing::new(Vec::new());
    samples
        .try_reserve_exact(a.sample_count as usize)
        .map_err(|_| Error::ResourceExhausted)?;
    for block in bytes.chunks(8192) {
        check()?;
        for b in block.chunks_exact(2) {
            samples.push(i16::from_le_bytes([b[0], b[1]]));
        }
    }
    check()?;
    Ok(PcmReader { samples })
}
impl Bank {
    fn harvest(&mut self) {
        let mut i = 0;
        while i < self.jobs.len() {
            match self.jobs[i].rx.try_recv() {
                Ok(result) => {
                    let valid = self.jobs[i].op.check().is_ok();
                    self.jobs.remove(i);
                    match result {
                        Ok(entry)
                            if valid
                                && entry.until > Instant::now()
                                && self.fence.check(entry.generation).is_ok() =>
                        {
                            self.items.push(entry)
                        }
                        Err(e) => eprintln!("MEDIA_PREPARATION_UNAVAILABLE: {}", e.as_str_name()),
                        _ => (),
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.jobs.remove(i);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    if self.jobs[i].op.check().is_err() {
                        self.jobs.remove(i);
                    } else {
                        i += 1;
                    }
                }
            }
        }
    }
    pub fn new(root: &Path, fence: Arc<StopFence>) -> Self {
        Self {
            root: root.into(),
            fence,
            items: Vec::new(),
            jobs: Vec::new(),
            active: Arc::new(AtomicUsize::new(0)),
        }
    }
    pub fn retain(&mut self, refs: &[(String, String)]) {
        self.harvest();
        self.jobs.retain(|j| {
            j.until > Instant::now() && refs.iter().any(|(ws, id)| *ws == j.ws && *id == j.id)
        });
        self.items.retain(|e| {
            e.until > Instant::now() && refs.iter().any(|(ws, id)| *ws == e.ws && *id == e.id)
        });
    }
    pub fn clear(&mut self) {
        self.items.clear();
        self.jobs.clear();
    }
    pub fn prepare(&mut self, op: &Operation, source: Source<'_>) -> Result<()> {
        uuid(source.workspace)?;
        uuid(source.task)?;
        op.check()?;
        if source.samples.audio.artifact_id != source.task
            || source.task_sha.len() != 64
            || !source.task_sha.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(Error::InvalidArgument);
        }
        expected_frames(source.samples.samples.len() as u64)?;
        self.harvest();
        if self.active.load(Ordering::Acquire) >= 2 {
            return Err(Error::ResourceExhausted);
        }
        if self.items.len() + self.jobs.len() >= 2 {
            return Err(Error::ResourceExhausted);
        }
        if self
            .items
            .iter()
            .any(|e| e.ws == source.workspace && e.id == source.task)
            || self
                .jobs
                .iter()
                .any(|j| j.ws == source.workspace && j.id == source.task)
        {
            return Err(Error::AlreadyExists);
        }
        let generation = self.fence.generation();
        self.fence.check(generation)?;
        let mut pcm = Zeroizing::new(Vec::new());
        pcm.try_reserve_exact(source.samples.samples.len())
            .map_err(|_| Error::ResourceExhausted)?;
        for chunk in source.samples.samples.chunks(4096) {
            op.check()?;
            self.fence.check(generation)?;
            pcm.extend_from_slice(chunk);
        }
        let ws = source.workspace.to_owned();
        let id = source.task.to_owned();
        let sha = source.task_sha.to_owned();
        let until = source.samples.until;
        let audio = source.samples.audio;
        let work = Operation::new(4000)?;
        let child_op = work.clone();
        let root = self.root.clone();
        let fence = self.fence.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let child_ws = ws.clone();
        let child_id = id.clone();
        let active = self.active.clone();
        active.fetch_add(1, Ordering::AcqRel);
        let permit = WorkerPermit(active);
        std::thread::Builder::new()
            .name("nai-media-prepare".into())
            .spawn(move || {
                let _permit = permit;
                let source = Source {
                    workspace: &child_ws,
                    task: &child_id,
                    task_sha: &sha,
                    samples: super::tts_audio::PreparationSamples {
                        audio,
                        samples: &pcm,
                        until,
                    },
                };
                let result = prepare_entry(root, fence.clone(), generation, &child_op, source);
                #[cfg(not(test))]
                let result = result.and_then(|mut e| {
                    let check = || {
                        child_op.check()?;
                        fence.check(generation)?;
                        if Instant::now() >= e.until {
                            return Err(Error::DeadlineExceeded);
                        }
                        Ok(())
                    };
                    e.receipt = Some(super::media_pipe::verify(
                        e.file.try_clone().map_err(|_| Error::IoFailed)?,
                        &e.view,
                        &check,
                    )?);
                    Ok(e)
                });
                let _ = tx.send(result);
            })
            .map_err(|_| Error::CapabilityUnavailable)?;
        self.jobs.push(Job {
            ws,
            id,
            until,
            op: work,
            rx,
        });
        Ok(())
    }
    pub fn export(
        &mut self,
        op: &Operation,
        ws: &str,
        id: &str,
        sha: &str,
        visual: &PublicMediaVisual,
    ) -> Result<(tempfile::NamedTempFile, PreparedMediaDescriptor)> {
        let (view, _) = self.observe(op, ws, id, sha, visual)?;
        let e = self
            .items
            .iter()
            .find(|e| e.ws == ws && e.id == id)
            .ok_or(Error::CapabilityUnavailable)?;
        let mut out = tempfile::Builder::new()
            .prefix("public-preload-")
            .suffix(".pcm")
            .make_in(&self.root, |path| {
                let mut options = std::fs::OpenOptions::new();
                options.read(true).write(true).create_new(true);
                #[cfg(windows)]
                {
                    use std::os::windows::fs::OpenOptionsExt;
                    options
                        .access_mode(0xc0010000)
                        .share_mode(7)
                        .custom_flags(0x04000000);
                }
                options.open(path)
            })
            .map_err(|_| Error::IoFailed)?;
        let reader = read(&e.file, &view, &|| op.check())?;
        let mut bytes = Zeroizing::new([0u8; 8192]);
        for chunk in reader.samples().chunks(4096) {
            op.check()?;
            self.fence.check(e.generation)?;
            if Instant::now() >= e.until {
                return Err(Error::DeadlineExceeded);
            }
            for (n, b) in chunk.iter().zip(bytes.chunks_exact_mut(2)) {
                b.copy_from_slice(&n.to_le_bytes());
            }
            out.write_all(&bytes[..chunk.len() * 2])
                .map_err(|_| Error::IoFailed)?;
        }
        out.flush().map_err(|_| Error::IoFailed)?;
        op.check()?;
        if Instant::now() >= e.until {
            return Err(Error::DeadlineExceeded);
        }
        Ok((out, view))
    }
    pub fn observe(
        &mut self,
        op: &Operation,
        ws: &str,
        id: &str,
        sha: &str,
        visual: &PublicMediaVisual,
    ) -> Result<(PreparedMediaDescriptor, Option<PreparedMediaReceipt>)> {
        self.harvest();
        let index = self
            .items
            .iter()
            .position(|e| e.ws == ws && e.id == id)
            .ok_or(Error::CapabilityUnavailable)?;
        let result = (|| {
            let e = &self.items[index];
            let check = || {
                op.check()?;
                self.fence.check(e.generation)?;
                if Instant::now() >= e.until {
                    return Err(Error::DeadlineExceeded);
                }
                Ok(())
            };
            check()?;
            if e.sha != sha || e.view.visual.as_ref() != Some(visual) {
                return Err(Error::RevisionConflict);
            }
            read(&e.file, &e.view, &check)?;
            check()?;
            Ok((e.view.clone(), e.receipt.clone()))
        })();
        if result.is_err() {
            self.items.remove(index);
        }
        result
    }
}

fn prepare_entry(
    root: PathBuf,
    fence: Arc<StopFence>,
    generation: u64,
    op: &Operation,
    source: Source<'_>,
) -> Result<Entry> {
    let check = || {
        op.check()?;
        fence.check(generation)?;
        if Instant::now() >= source.samples.until {
            return Err(Error::DeadlineExceeded);
        }
        Ok(())
    };
    check()?;
    let avatar = BuiltinAvatar::new(&check)?;
    let visual = avatar.observe(&check)?;
    let mut renderer = PublicClipRenderer::new(
        source.samples.samples,
        &source.samples.audio,
        &avatar,
        &check,
    )?;
    let first = renderer
        .frame_at(0, &check)?
        .ok_or(Error::IntegrityFailed)?;
    let mut file = tempfile::tempfile_in(&root).map_err(|_| Error::IoFailed)?;
    let mut buffer = Zeroizing::new([0u8; 8192]);
    for chunk in source.samples.samples.chunks(4096) {
        check()?;
        for (s, b) in chunk.iter().zip(buffer.chunks_exact_mut(2)) {
            b.copy_from_slice(&s.to_le_bytes());
        }
        file.write_all(&buffer[..chunk.len() * 2])
            .map_err(|_| Error::IoFailed)?;
    }
    file.sync_all().map_err(|_| Error::IoFailed)?;
    check()?;
    let view = PreparedMediaDescriptor {
        protocol_major: 1,
        workspace_id: source.workspace.into(),
        task_id: source.task.into(),
        task_sha256: source.task_sha.into(),
        audio: Some(source.samples.audio),
        visual: Some(visual),
        expected_frames: expected_frames(source.samples.samples.len() as u64)?,
        samples_per_frame: FRAME_SAMPLES as u32,
        first_frame_sha256: first.info.pixels_sha256,
        pcm_format: "s16le-mono-24000".into(),
        pixel_format: "rgb32le".into(),
        renderer_id: RENDERER_ID.into(),
    };
    // Verify the actual data plane before publishing even a preparation descriptor.
    let reader = read(&file, &view, &check)?;
    let mut consumer = PublicClipRenderer::new(
        reader.samples(),
        view.audio.as_ref().ok_or(Error::IntegrityFailed)?,
        &avatar,
        &check,
    )?;
    if consumer
        .frame_at(0, &check)?
        .ok_or(Error::IntegrityFailed)?
        .info
        .pixels_sha256
        != view.first_frame_sha256
    {
        return Err(Error::IntegrityFailed);
    }
    check()?;
    Ok(Entry {
        ws: source.workspace.into(),
        id: source.task.into(),
        sha: source.task_sha.into(),
        file,
        view,
        receipt: None,
        until: source.samples.until,
        generation,
    })
}

#[cfg(test)]
mod tests {
    use super::super::tts_audio::Cache;
    use super::*;
    const WS: &str = "00000000-0000-4000-8000-000000000001";
    fn id(n: u64) -> String {
        format!("00000000-0000-4000-8000-{n:012}")
    }
    fn cache(fence: Arc<StopFence>, n: u64) -> Cache {
        let mut c = Cache::new(1, fence);
        let pcm = vec![12000; 1601];
        let sha = nai_media_core::public_clip::pcm_hash(&pcm, &|| Ok(())).unwrap();
        c.insert(
            WS,
            &id(n),
            &"a".repeat(64),
            &sha,
            Zeroizing::new(pcm),
            Instant::now(),
        )
        .unwrap();
        c
    }
    fn prepare(bank: &mut Bank, cache: &Cache, n: u64) -> Result<()> {
        let op = Operation::new(4000)?;
        bank.prepare(
            &op,
            Source {
                workspace: WS,
                task: &id(n),
                task_sha: &"a".repeat(64),
                samples: cache.preparation_samples(
                    WS,
                    &id(n),
                    &"a".repeat(64),
                    Instant::now(),
                    &op,
                )?,
            },
        )?;
        let until = Instant::now() + std::time::Duration::from_secs(5);
        loop {
            bank.harvest();
            if bank.items.iter().any(|e| e.id == id(n)) {
                return Ok(());
            }
            if bank.jobs.is_empty() || Instant::now() >= until {
                return Err(Error::DeadlineExceeded);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    #[test]
    fn actual_anonymous_data_round_trip_renders_the_exact_tail_without_control_bytes() {
        let t = tempfile::tempdir().unwrap();
        let f = Arc::new(StopFence::default());
        let cache = cache(f.clone(), 2);
        let mut bank = Bank::new(t.path(), f);
        prepare(&mut bank, &cache, 2).unwrap();
        let entry = &bank.items[0];
        let reader = read(&entry.file, &entry.view, &|| Ok(())).unwrap();
        assert_eq!(reader.samples(), vec![12000; 1601]);
        let avatar = BuiltinAvatar::new(&|| Ok(())).unwrap();
        let mut r = PublicClipRenderer::new(
            reader.samples(),
            entry.view.audio.as_ref().unwrap(),
            &avatar,
            &|| Ok(()),
        )
        .unwrap();
        let first = r.frame_at(0, &|| Ok(())).unwrap().unwrap();
        assert_eq!(first.info.pixels_sha256, entry.view.first_frame_sha256);
        assert_eq!(
            r.frame_at(1600, &|| Ok(()))
                .unwrap()
                .unwrap()
                .info
                .sample_count,
            1
        );
        assert_eq!(entry.view.expected_frames, 3);
        assert_eq!(entry.view.pcm_format, "s16le-mono-24000");
        let view = bank
            .observe(
                &Operation::new(4000).unwrap(),
                WS,
                &id(2),
                &"a".repeat(64),
                &avatar.observe(&|| Ok(())).unwrap(),
            )
            .unwrap();
        assert_eq!(view.0.first_frame_sha256.len(), 64);
    }
    #[test]
    fn corruption_cancel_expiry_and_release_do_not_return_old_prepared_descriptors() {
        for failure in 0..4 {
            let t = tempfile::tempdir().unwrap();
            let f = Arc::new(StopFence::default());
            let mut c = cache(f.clone(), 2);
            let mut bank = Bank::new(t.path(), f.clone());
            prepare(&mut bank, &c, 2).unwrap();
            let visual = bank.items[0].view.visual.clone().unwrap();
            match failure {
                0 => {
                    bank.items[0].file.seek(SeekFrom::Start(0)).unwrap();
                    bank.items[0].file.write_all(&[0, 0]).unwrap();
                }
                1 => f.stop(),
                2 => bank.items[0].until = Instant::now(),
                _ => {
                    c.revoke(WS, &id(2));
                    bank.retain(&c.references());
                }
            }
            assert!(
                bank.observe(
                    &Operation::new(4000).unwrap(),
                    WS,
                    &id(2),
                    &"a".repeat(64),
                    &visual
                )
                .is_err()
            );
            assert!(bank.items.is_empty());
        }
    }
    #[test]
    fn prepared_file_capacity_is_bounded_and_clearing_does_not_change_private_pcm() {
        let t = tempfile::tempdir().unwrap();
        let f = Arc::new(StopFence::default());
        let c = cache(f.clone(), 2);
        let c2 = cache(f.clone(), 3);
        let c3 = cache(f.clone(), 4);
        let mut bank = Bank::new(t.path(), f);
        prepare(&mut bank, &c, 2).unwrap();
        prepare(&mut bank, &c2, 3).unwrap();
        assert_eq!(prepare(&mut bank, &c3, 4), Err(Error::ResourceExhausted));
        bank.clear();
        assert!(
            c.observe(
                WS,
                &id(2),
                &"a".repeat(64),
                Instant::now(),
                &Operation::new(4000).unwrap()
            )
            .is_ok()
        );
        assert!(bank.items.is_empty());
    }
    #[test]
    fn clearing_pending_work_cancels_and_cannot_publish_after_release() {
        let t = tempfile::tempdir().unwrap();
        let f = Arc::new(StopFence::default());
        let c = cache(f.clone(), 2);
        let mut bank = Bank::new(t.path(), f);
        let op = Operation::new(4000).unwrap();
        bank.prepare(
            &op,
            Source {
                workspace: WS,
                task: &id(2),
                task_sha: &"a".repeat(64),
                samples: c
                    .preparation_samples(WS, &id(2), &"a".repeat(64), Instant::now(), &op)
                    .unwrap(),
            },
        )
        .unwrap();
        bank.clear();
        let until = Instant::now() + std::time::Duration::from_secs(5);
        while bank.active.load(Ordering::Acquire) > 0 {
            assert!(Instant::now() < until, "cancelled worker did not retire");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        bank.harvest();
        assert!(bank.items.is_empty());
        assert!(bank.jobs.is_empty());
    }
}
