//! Local encoder with an inherited media input and loopback MPEG-TS output.
use std::{
    io::{self, Write},
    os::windows::process::CommandExt,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

const WRITE_WAIT: Duration = Duration::from_millis(250);
const FINISH_WAIT: Duration = Duration::from_secs(2);
const QUEUE_LIMIT: usize = 8;
const FRAME_PERIOD: Duration = Duration::from_millis(40);
const LEASE_RENEW: Duration = Duration::from_millis(100);
pub const OBS_PREROLL_FRAMES: u32 = 75;
const OBS_TAIL_FRAMES: u32 = 50;
pub const PCM_BYTES_PER_FRAME: usize = 960 * 2;
type Packet = Zeroizing<Vec<u8>>;

pub struct Encoder {
    child: Child,
    input: Option<mpsc::SyncSender<(Packet, Packet)>>,
    valid: Arc<AtomicBool>,
    frame_bytes: usize,
    next_idle: Instant,
    lease: Option<RecordingLease>,
    stop_drill: Option<(
        nai_runtime_core::foundation::Operation,
        Duration,
        Option<Instant>,
    )>,
}

struct RecordingLease {
    path: std::path::PathBuf,
    nonce: String,
    sequence: u32,
    next_renew: Instant,
}

impl RecordingLease {
    fn write(&self, sequence: u32) -> io::Result<()> {
        let pending = self.path.with_extension("lease.tmp");
        std::fs::write(&pending, format!("{}:{sequence:08}", self.nonce))?;
        std::fs::rename(pending, &self.path)
    }
}

impl Encoder {
    pub fn start(ffmpeg: &Path, width: u32, height: u32, port: u16) -> io::Result<Self> {
        if !ffmpeg.is_absolute()
            || port < 1024
            || ![width, height]
                .into_iter()
                .all(|size| (16..=1024).contains(&size) && size.is_multiple_of(2))
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let output = format!("udp://127.0.0.1:{port}?pkt_size=1316");
        let valid = Arc::new(AtomicBool::new(true));
        let mut child = Command::new(ffmpeg)
            .args([
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "warning",
                "-thread_queue_size",
                "8",
                "-probesize",
                "32",
                "-analyzeduration",
                "1",
                "-f",
                "avi",
                "-i",
                "pipe:0",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-tune",
                "zerolatency",
                "-pix_fmt",
                "yuv420p",
                "-g",
                "25",
                "-c:a",
                "aac",
                "-muxdelay",
                "0",
                "-f",
                "mpegts",
                &output,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()?;
        let Some(stdin) = child.stdin.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("encoder input unavailable"));
        };
        let input = writer(stdin, width, height, valid.clone());
        Ok(Self {
            child,
            input: Some(input),
            valid,
            frame_bytes: width as usize * height as usize * 4,
            next_idle: Instant::now(),
            lease: None,
            stop_drill: None,
        })
    }

    /// Inject a task cancellation only after real playback is observed.
    pub fn arm_stop_drill(
        &mut self,
        operation: nai_runtime_core::foundation::Operation,
        after: Duration,
    ) {
        self.stop_drill = Some((operation, after, None));
    }

    fn check_stop_drill(&mut self) -> io::Result<()> {
        if let Some((operation, after, first)) = &mut self.stop_drill {
            let started = first.get_or_insert_with(Instant::now);
            if started.elapsed() >= *after {
                operation.cancel();
                eprintln!(
                    "OBS_STOP_INJECTED after_first_pair_ms={}",
                    started.elapsed().as_millis()
                );
                self.stop();
                return Err(io::Error::new(io::ErrorKind::Interrupted, "OBS stop drill"));
            }
        }
        Ok(())
    }

    /// Use only with the dedicated recorder's one-run acknowledgement.
    pub fn bind_recording_lease(&mut self, ready: &Path, nonce: &str) -> io::Result<()> {
        if self.lease.is_some()
            || !ready.is_absolute()
            || nonce.len() != 32
            || !nonce
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let path = ready.with_file_name("media.lease");
        if path.exists() || path.with_extension("lease.tmp").exists() {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        self.lease = Some(RecordingLease {
            path,
            nonce: nonce.to_owned(),
            sequence: 0,
            next_renew: Instant::now(),
        });
        Ok(())
    }

    pub fn submit(&mut self, video: Packet, pcm: Packet) -> io::Result<()> {
        if video.len() != self.frame_bytes || pcm.len() != PCM_BYTES_PER_FRAME {
            self.stop();
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let failure = if !self.valid.load(Ordering::Acquire) || self.child.try_wait()?.is_some() {
            Some("encoder input or process unavailable")
        } else if self
            .input
            .as_ref()
            .is_none_or(|input| input.try_send((video, pcm)).is_err())
        {
            Some("encoder media input stalled")
        } else {
            None
        };
        if let Some(failure) = failure {
            self.stop();
            return Err(io::Error::other(failure));
        }
        if let Some(lease) = &mut self.lease
            && Instant::now() >= lease.next_renew
        {
            lease.sequence = lease
                .sequence
                .checked_add(1)
                .filter(|n| *n <= 99_999_999)
                .ok_or_else(|| io::Error::other("OBS lease sequence exhausted"))?;
            lease.write(lease.sequence)?;
            lease.next_renew = Instant::now() + LEASE_RENEW;
        }
        Ok(())
    }

    pub fn stop(&mut self) {
        self.valid.store(false, Ordering::Release);
        if let Some(lease) = self.lease.take() {
            let _ = lease.write(0);
        }
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Keep OBS's demuxer active while the source is silent. Send at media rate.
    pub fn send_black_silence(&mut self, frames: u32) -> io::Result<()> {
        let began = Instant::now();
        self.next_idle = began;
        for _ in 0..frames {
            thread::sleep(self.next_idle.saturating_duration_since(Instant::now()));
            self.keep_idle()?;
        }
        thread::sleep(self.next_idle.saturating_duration_since(Instant::now()));
        Ok(())
    }

    pub(crate) fn keep_idle(&mut self) -> io::Result<()> {
        self.keep_silent_image(None)
    }

    fn keep_silent_image(&mut self, pixels: Option<&[u32]>) -> io::Result<()> {
        let now = Instant::now();
        if now >= self.next_idle {
            if let Some(pixels) = pixels.filter(|pixels| pixels.len() == self.frame_bytes / 4) {
                let bytes = Zeroizing::new(
                    pixels
                        .iter()
                        .flat_map(|pixel| pixel.to_le_bytes())
                        .collect(),
                );
                self.submit(bytes, Zeroizing::new(vec![0; PCM_BYTES_PER_FRAME]))?;
            } else {
                self.submit_black_silence()?;
            }
            let due = self.next_idle + FRAME_PERIOD;
            self.next_idle = if due <= now { now + FRAME_PERIOD } else { due };
        }
        Ok(())
    }

    fn submit_black_silence(&mut self) -> io::Result<()> {
        self.submit(
            Zeroizing::new(vec![0; self.frame_bytes]),
            Zeroizing::new(vec![0; PCM_BYTES_PER_FRAME]),
        )
    }

    /// Only use after the media producer has confirmed a complete input.
    /// Dropping the sender drains queued pairs before the writer closes stdin.
    pub fn finish(mut self) -> io::Result<()> {
        self.input.take();
        let began = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait()? {
                return if status.success() && self.valid.load(Ordering::Acquire) {
                    Ok(())
                } else {
                    Err(io::Error::other("encoder failed while draining"))
                };
            }
            if began.elapsed() >= FINISH_WAIT {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "encoder drain timed out",
                ));
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Consume the optional production subscription on a dedicated thread. A gap
/// stops this output so the encoder's fixed-rate timestamps cannot hide it.
pub fn forward(
    observer: crate::v561_stream_output::OutputObserver,
    mut encoder: Encoder,
) -> io::Result<u64> {
    let (frames, completed) = forward_segment(observer, &mut encoder)?;
    if completed {
        encoder.finish_with_tail()?;
    }
    Ok(frames)
}

impl Encoder {
    pub fn finish_with_tail(mut self) -> io::Result<()> {
        self.send_black_silence(OBS_TAIL_FRAMES)?;
        self.finish()
    }
}

pub(crate) fn forward_segment(
    observer: crate::v561_stream_output::OutputObserver,
    encoder: &mut Encoder,
) -> io::Result<(u64, bool)> {
    forward_segment_with_standby(observer, encoder, None)
}

pub(crate) fn forward_segment_with_standby(
    observer: crate::v561_stream_output::OutputObserver,
    encoder: &mut Encoder,
    standby: Option<(
        &crate::v561_standby::View,
        &nai_runtime_core::foundation::Operation,
    )>,
) -> io::Result<(u64, bool)> {
    use crate::v561_obs_timeline::Timeline;
    let mut timeline = None;
    let mut next = 0u64;
    let mut tail = None;
    let mut last_submit: Option<Instant> = None;
    let began = Instant::now();
    let samples = (PCM_BYTES_PER_FRAME / 2) as u64;
    let invalid = |error| io::Error::other(format!("OBS media timeline: {error:?}"));
    loop {
        while let Ok(block) = observer.pcm.try_recv() {
            let Some((epoch, generation, offset, bytes)) = block.into_parts() else {
                return Ok((next / samples, false));
            };
            let timeline = timeline.get_or_insert_with(|| Timeline::new(epoch, generation));
            timeline
                .as_mut()
                .ok_or_else(|| io::Error::other("invalid media identity"))?
                .append(epoch, generation, offset, &bytes)
                .map_err(invalid)?;
        }
        let waiting_for_first = last_submit.is_none() && next == 0 && tail.is_none();
        let mut wait = Duration::from_millis(10);
        if waiting_for_first {
            wait = wait.min(encoder.next_idle.saturating_duration_since(Instant::now()));
        }
        let selected = match observer.frames.recv_timeout(wait) {
            Ok(selected) => selected,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if waiting_for_first {
                    if began.elapsed() >= Duration::from_secs(45) {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "OBS first frame unavailable",
                        ));
                    }
                    let pixels = if let Some((view, operation)) = standby {
                        if operation.check().is_err() {
                            return Ok((next / samples, false));
                        }
                        view.pixels()
                    } else {
                        None
                    };
                    encoder.keep_silent_image(pixels)?;
                }
                if last_submit.is_some_and(|at| at.elapsed() >= Duration::from_millis(500)) {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!(
                            "OBS selected frames expired after {} submitted pairs at {} ms",
                            next / samples,
                            began.elapsed().as_millis()
                        ),
                    ));
                }
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        // A frame can arrive while PCM is enqueued on the other thread.
        while let Ok(block) = observer.pcm.try_recv() {
            let Some((epoch, generation, offset, bytes)) = block.into_parts() else {
                return Ok((next / samples, false));
            };
            timeline
                .get_or_insert_with(|| Timeline::new(epoch, generation))
                .as_mut()
                .ok_or_else(|| io::Error::other("invalid media identity"))?
                .append(epoch, generation, offset, &bytes)
                .map_err(invalid)?;
        }
        let Some((frame, clock)) = selected.into_fresh_parts() else {
            return Ok((next / samples, false));
        };
        let timeline = timeline
            .as_mut()
            .and_then(Option::as_mut)
            .ok_or_else(|| io::Error::other("no PCM for selected frame"))?;
        let selected_sample = timeline
            .frame_start(frame.metadata(), &clock)
            .map_err(invalid)?;
        if selected_sample != next {
            return Err(io::Error::other(format!(
                "missing selected frame: expected sample {next}, selected sample {selected_sample}"
            )));
        }
        let end = next + samples;
        let pcm = match timeline.take_until(end) {
            Ok(pcm) => pcm,
            Err(crate::v561_obs_timeline::Error::Unavailable) => {
                tail = Some(Zeroizing::new(frame.into_frame().pixels));
                continue;
            }
            Err(error) => return Err(invalid(error)),
        };
        let bytes = Zeroizing::new(pcm.iter().flat_map(|sample| sample.to_le_bytes()).collect());
        encoder.check_stop_drill()?;
        encoder.submit(Zeroizing::new(frame.into_frame().pixels), bytes)?;
        encoder.next_idle = Instant::now() + FRAME_PERIOD;
        if last_submit.is_none() {
            eprintln!(
                "OBS_FIRST_PAIR encoder_elapsed_ms={} played_sample={:?}",
                began.elapsed().as_millis(),
                clock.played_sample
            );
        }
        last_submit = Some(Instant::now());
        next = end;
    }
    let Ok(total) = observer.completed.try_recv() else {
        return Ok((next / samples, false));
    };
    let mut frames = next / samples;
    match total.checked_sub(next) {
        Some(0) if frames > 0 && tail.is_none() => {}
        Some(remaining) if (1..samples).contains(&remaining) => {
            let frame = tail.ok_or_else(|| io::Error::other("missing final OBS frame"))?;
            let timeline = timeline
                .as_mut()
                .and_then(Option::as_mut)
                .ok_or_else(|| io::Error::other("missing final OBS audio"))?;
            let pcm = timeline.take_until(total).map_err(invalid)?;
            let bytes = pad_tail(&pcm)?;
            encoder.submit(frame, bytes)?;
            encoder.next_idle = Instant::now() + FRAME_PERIOD;
            frames += 1;
        }
        _ => return Err(io::Error::other("OBS completion sample count mismatch")),
    }
    eprintln!(
        "OBS_DRAIN_START encoder_elapsed_ms={} frames={frames} source_samples={total}",
        began.elapsed().as_millis()
    );
    Ok((frames, true))
}

fn pad_tail(pcm: &[i16]) -> io::Result<Packet> {
    if pcm.is_empty() || pcm.len() >= PCM_BYTES_PER_FRAME / 2 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut bytes = Zeroizing::new(vec![0; PCM_BYTES_PER_FRAME]);
    for (destination, sample) in bytes.chunks_exact_mut(2).zip(pcm) {
        destination.copy_from_slice(&sample.to_le_bytes());
    }
    Ok(bytes)
}

fn writer(
    mut stdin: impl Write + Send + 'static,
    width: u32,
    height: u32,
    valid: Arc<AtomicBool>,
) -> mpsc::SyncSender<(Packet, Packet)> {
    let (send, receive) = mpsc::sync_channel::<(Packet, Packet)>(QUEUE_LIMIT);
    thread::spawn(move || {
        let mut run = || -> io::Result<()> {
            stdin.write_all(&avi_header(width, height))?;
            while valid.load(Ordering::Acquire) {
                match receive.recv_timeout(WRITE_WAIT) {
                    Ok((frame, pcm)) if valid.load(Ordering::Acquire) => {
                        put_chunk(&mut stdin, b"00db", &frame)?;
                        if valid.load(Ordering::Acquire) {
                            put_chunk(&mut stdin, b"01wb", &pcm)?;
                        }
                    }
                    Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
            Ok(())
        };
        if run().is_err() {
            valid.store(false, Ordering::Release);
        }
    });
    send
}

fn put_chunk(output: &mut impl Write, kind: &[u8; 4], data: &[u8]) -> io::Result<()> {
    output.write_all(kind)?;
    output.write_all(&(data.len() as u32).to_le_bytes())?;
    output.write_all(data)?;
    if !data.len().is_multiple_of(2) {
        output.write_all(&[0])?;
    }
    Ok(())
}

fn avi_header(width: u32, height: u32) -> Vec<u8> {
    fn dwords(values: &[u32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }
    let bytes = width * height * 4;
    let mut header = b"hdrl".to_vec();
    let main = dwords(&[
        40_000,
        bytes * 25 + 48_000,
        0,
        0x100,
        0,
        0,
        2,
        bytes,
        width,
        height,
        0,
        0,
        0,
        0,
    ]);
    put_chunk(&mut header, b"avih", &main).unwrap();
    let mut video = b"strl".to_vec();
    let mut video_stream = b"vidsDIB ".to_vec();
    video_stream.extend(dwords(&[
        0,
        0,
        0,
        1,
        25,
        0,
        0,
        bytes,
        u32::MAX,
        0,
        0,
        width | (height << 16),
    ]));
    put_chunk(&mut video, b"strh", &video_stream).unwrap();
    let format = dwords(&[
        40,
        width,
        (-(height as i32)) as u32,
        (32 << 16) | 1,
        0,
        bytes,
        0,
        0,
        0,
        0,
    ]);
    put_chunk(&mut video, b"strf", &format).unwrap();
    put_chunk(&mut header, b"LIST", &video).unwrap();
    let mut audio = b"strl".to_vec();
    let mut audio_stream = b"auds\0\0\0\0".to_vec();
    audio_stream.extend(dwords(&[
        0,
        0,
        0,
        2,
        48_000,
        0,
        0,
        PCM_BYTES_PER_FRAME as u32,
        u32::MAX,
        2,
        0,
        0,
    ]));
    put_chunk(&mut audio, b"strh", &audio_stream).unwrap();
    put_chunk(
        &mut audio,
        b"strf",
        &dwords(&[0x00010001, 24_000, 48_000, 0x00100002]),
    )
    .unwrap();
    put_chunk(&mut header, b"LIST", &audio).unwrap();
    let mut riff = b"RIFF\xff\xff\xff\xffAVI ".to_vec();
    put_chunk(&mut riff, b"LIST", &header).unwrap();
    riff.extend_from_slice(b"LIST\xff\xff\xff\xffmovi");
    riff
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, UdpSocket};

    #[test]
    fn final_pcm_preserves_samples_and_pads_only_the_tail() {
        let padded = pad_tail(&[321, -123]).unwrap();
        assert_eq!(&padded[..4], &[65, 1, 133, 255]);
        assert!(padded[4..].iter().all(|&byte| byte == 0));
        assert!(pad_tail(&[]).is_err());
        assert!(pad_tail(&[0; PCM_BYTES_PER_FRAME / 2]).is_err());
    }

    #[test]
    #[ignore = "requires an explicit local FFmpeg binary and output directory"]
    fn loopback_fixture_encodes_video_and_pcm() {
        let binary = std::path::PathBuf::from(std::env::var_os("NAI_OBS_TEST_FFMPEG").unwrap());
        let output = std::path::PathBuf::from(std::env::var_os("NAI_OBS_TEST_TRANSPORT").unwrap());
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        socket.set_nonblocking(true).unwrap();
        let mut encoder =
            Encoder::start(&binary, 128, 128, socket.local_addr().unwrap().port()).unwrap();
        let mut capture = std::fs::File::create_new(&output).unwrap();
        let mut datagram = [0; 1316];
        let mut receive_packets = || {
            loop {
                match socket.recv(&mut datagram) {
                    Ok(size) => capture.write_all(&datagram[..size]).unwrap(),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(error) => panic!("UDP receive: {error}"),
                }
            }
        };
        for index in 0..75 {
            let video = Zeroizing::new(vec![index; 128 * 128 * 4]);
            let pcm = Zeroizing::new(vec![0; PCM_BYTES_PER_FRAME]);
            encoder.submit(video, pcm).unwrap();
            thread::sleep(Duration::from_millis(40));
            receive_packets();
        }
        encoder.finish().unwrap();
        receive_packets();
        drop(capture);
        let decoded = Command::new(binary)
            .args(["-v", "error", "-i"])
            .arg(output)
            .args([
                "-map",
                "0:v:0",
                "-map",
                "0:a:0",
                "-progress",
                "pipe:1",
                "-f",
                "null",
                "-",
            ])
            .output()
            .unwrap();
        assert!(
            decoded.status.success(),
            "{}",
            String::from_utf8_lossy(&decoded.stderr)
        );
        assert!(
            String::from_utf8_lossy(&decoded.stdout)
                .lines()
                .any(|line| line == "frame=75")
        );
    }
}
