//! One resident native stream; COM interfaces remain on their owning thread.
use crate::local_audio::{Packet, Queue};
use nai_media_core::device_clock::DeviceClockCursor;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

pub(crate) struct AudioSession {
    pub tx: mpsc::SyncSender<Packet>,
    pub channels: usize,
    pub open_count: u32,
    reason: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
}
static STREAM_OPENS: AtomicU64 = AtomicU64::new(0);
struct ThreadHealth(Arc<AtomicU64>, Arc<AtomicBool>);
impl Drop for ThreadHealth {
    fn drop(&mut self) {
        if !self.1.load(Ordering::SeqCst) {
            let _ = self
                .0
                .compare_exchange(0, 5, Ordering::SeqCst, Ordering::SeqCst);
        }
    }
}
impl Drop for AudioSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}
impl AudioSession {
    pub fn health(&self) -> Result<(), &'static str> {
        match self.reason.load(Ordering::SeqCst) {
            0 => Ok(()),
            1 => Err("DEVICE_AUDIO_TIMING_UNAVAILABLE"),
            2 => Err("DEVICE_AUDIO_BUFFER_TOO_LARGE"),
            3 => Err("DEVICE_AUDIO_CHUNK_REJECTED"),
            4 => Err("DEVICE_AUDIO_FINAL_DEADLINE"),
            6 => Err("DEVICE_AUDIO_POSITION_INVALID"),
            _ => Err("DEVICE_AUDIO_STREAM_ERROR"),
        }
    }
    pub fn open() -> Result<Self, &'static str> {
        let (tx, rx) = mpsc::sync_channel(1);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let reason = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_reason = reason.clone();
        let thread_stop = stop.clone();
        std::thread::Builder::new()
            .name("nai-wasapi-output".into())
            .spawn(move || {
                let _health = ThreadHealth(thread_reason.clone(), thread_stop.clone());
                let outcome = drive(rx, &thread_stop, &ready_tx);
                if let Err(code) = outcome {
                    thread_reason.store(code, Ordering::SeqCst);
                    let _ = ready_tx.try_send(Err("DEVICE_STREAM_OPEN_FAILED"));
                }
            })
            .map_err(|_| "DEVICE_STREAM_OPEN_FAILED")?;
        match ready_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(())) => {}
            result => {
                stop.store(true, Ordering::SeqCst);
                return Err(result
                    .ok()
                    .and_then(Result::err)
                    .unwrap_or("DEVICE_STREAM_OPEN_TIMEOUT"));
            }
        }
        Ok(Self {
            tx,
            channels: 2,
            open_count: u32::try_from(STREAM_OPENS.fetch_add(1, Ordering::SeqCst) + 1)
                .map_err(|_| "DEVICE_STREAM_LIMIT")?,
            reason,
            stop,
        })
    }
}

fn drive(
    rx: mpsc::Receiver<Packet>,
    stop: &AtomicBool,
    ready: &mpsc::SyncSender<Result<(), &'static str>>,
) -> Result<(), u64> {
    wasapi::initialize_mta().ok().map_err(|_| 5u64)?;
    let enumerator = DeviceEnumerator::new().map_err(|_| 5u64)?;
    let device = enumerator
        .get_default_device(&Direction::Render)
        .map_err(|_| 5u64)?;
    let mut client = device.get_iaudioclient().map_err(|_| 5u64)?;
    let format = WaveFormat::new(32, 32, &SampleType::Float, 48000, 2, None);
    client
        .initialize_client(
            &format,
            &Direction::Render,
            &StreamMode::EventsShared {
                autoconvert: true,
                buffer_duration_hns: 200_000,
            },
        )
        .map_err(|_| 5u64)?;
    let event = client.set_get_eventhandle().map_err(|_| 5u64)?;
    let render = client.get_audiorenderclient().map_err(|_| 5u64)?;
    let clock = client.get_audioclock().map_err(|_| 6u64)?;
    let frequency = clock.get_frequency().map_err(|_| 6u64)?;
    if clock.get_position().map_err(|_| 6u64)?.0 != 0 {
        return Err(6);
    }
    let mut cursor = DeviceClockCursor::new(1, frequency, 48000).map_err(|_| 6u64)?;
    let mut queue = Queue::new(rx, 2);
    let mut floats = [0.0f32; 4800 * 2];
    let mut bytes = [0u8; 4800 * 8];
    let mut submitted = u64::from(client.get_available_space_in_frames().map_err(|_| 5u64)?);
    if submitted == 0 || submitted > 4800 {
        return Err(2);
    }
    render
        .write_to_device(submitted as usize, &bytes[..submitted as usize * 8], None)
        .map_err(|_| 5u64)?;
    cursor.commit(submitted).map_err(|_| 6u64)?;
    if stop.load(Ordering::SeqCst) {
        return Ok(());
    }
    client.start_stream().map_err(|_| 5u64)?;
    struct Stop<'a>(&'a wasapi::AudioClient);
    impl Drop for Stop<'_> {
        fn drop(&mut self) {
            let _ = self.0.stop_stream();
        }
    }
    let _stop = Stop(&client);
    ready.try_send(Ok(())).map_err(|_| 5u64)?;
    while !stop.load(Ordering::SeqCst) {
        event.wait_for_event(100).map_err(|_| 5u64)?;
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let position = clock.get_position().map_err(|_| 6u64)?.0;
        let played = cursor
            .observe(1, clock.get_frequency().map_err(|_| 6u64)?, position)
            .map_err(|_| 6u64)?;
        queue.observe_device(played, Instant::now());
        let frames = client.get_available_space_in_frames().map_err(|_| 5u64)? as usize;
        if frames == 0 {
            continue;
        }
        if frames > 4800 {
            return Err(2);
        }
        let latency = Duration::from_nanos(((submitted - played) * 1_000_000_000).div_ceil(48000));
        queue.fill(&mut floats[..frames * 2], Instant::now(), Some(latency))?;
        for (sample, raw) in floats[..frames * 2]
            .iter()
            .zip(bytes[..frames * 8].chunks_exact_mut(4))
        {
            raw.copy_from_slice(&sample.to_le_bytes());
        }
        if !queue.validate_device_write(Instant::now()) {
            bytes[..frames * 8].fill(0);
        }
        render
            .write_to_device(frames, &bytes[..frames * 8], None)
            .map_err(|_| 5u64)?;
        queue.commit_device_write(submitted)?;
        cursor.commit(frames as u64).map_err(|_| 6u64)?;
        submitted = submitted.checked_add(frames as u64).ok_or(6u64)?;
    }
    Ok(())
}
