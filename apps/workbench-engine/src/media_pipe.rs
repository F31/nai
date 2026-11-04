//! Fixed preparation child. Independent read-only PCM pipe, never a playback grant.
use nai_contracts::{
    audition_wire as wire,
    workbench::{PreparedMediaDescriptor, PreparedMediaReceipt, WorkbenchError as Error},
};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
type Result<T> = std::result::Result<T, Error>;
struct Reap(Child, Vec<std::thread::JoinHandle<()>>);
impl Drop for Reap {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
        for t in self.1.drain(..) {
            let _ = t.join();
        }
    }
}
fn binary() -> Result<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(p) = std::env::var_os("NAI_PUBLIC_OUTPUT_KEEPER") {
        let p = PathBuf::from(p);
        if p.is_absolute() && p.is_file() {
            return Ok(p);
        }
        return Err(Error::CapabilityUnavailable);
    }
    let p = std::env::current_exe()
        .map_err(|_| Error::CapabilityUnavailable)?
        .with_file_name(format!(
            "nai-public-output-keeper{}",
            std::env::consts::EXE_SUFFIX
        ));
    if p.is_file() {
        Ok(p)
    } else {
        Err(Error::CapabilityUnavailable)
    }
}
pub fn verify(
    file: File,
    d: &PreparedMediaDescriptor,
    check: &dyn Fn() -> Result<()>,
) -> Result<PreparedMediaReceipt> {
    check()?;
    verify_at(&binary()?, file, d, check)
}
pub(super) fn verify_at(
    binary: &Path,
    mut file: File,
    d: &PreparedMediaDescriptor,
    check: &dyn Fn() -> Result<()>,
) -> Result<PreparedMediaReceipt> {
    check()?;
    let instance = random_id()?;
    file.seek(SeekFrom::Start(0)).map_err(|_| Error::IoFailed)?;
    let mut command = Command::new(binary);
    command.env_clear();
    for name in ["SystemRoot", "WINDIR"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }

    command
        .arg("--verify-media")
        .env(
            "NAI_PREPARED_DESCRIPTOR",
            nai_contracts::prepared_descriptor_env(d).map_err(|_| Error::IntegrityFailed)?,
        )
        .env("NAI_PREPARED_CONSUMER", &instance)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = Reap(
        command.spawn().map_err(|_| Error::CapabilityUnavailable)?,
        Vec::new(),
    );
    let mut input = child.0.stdin.take().ok_or(Error::IoFailed)?;
    let mut output = child.0.stdout.take().ok_or(Error::IoFailed)?;
    let (tx, rx) = mpsc::sync_channel(1);
    let (rt, rr) = mpsc::sync_channel(1);
    child.1.push(std::thread::spawn(move || {
        let result = (|| {
            let mut b = zeroize::Zeroizing::new([0u8; 8192]);
            let mut file = file.take(960001);
            loop {
                let n = file.read(&mut *b)?;
                if n == 0 {
                    break;
                }
                input.write_all(&b[..n])?;
            }
            Ok::<(), std::io::Error>(())
        })();
        drop(input);
        let _ = tx.send(result);
    }));
    child.1.push(std::thread::spawn(move || {
        let result = wire::read::<PreparedMediaReceipt>(&mut output);
        let _ = rt.send(result);
    }));
    let until = std::time::Instant::now() + Duration::from_secs(4);
    let mut sent = false;
    let mut received = None;
    loop {
        check()?;
        if std::time::Instant::now() >= until {
            return Err(Error::DeadlineExceeded);
        }
        if !sent {
            match rx.try_recv() {
                Ok(Ok(())) => sent = true,
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => return Err(Error::IoFailed),
                Err(mpsc::TryRecvError::Empty) => (),
            }
        }
        if received.is_none() {
            match rr.try_recv() {
                Ok(Ok(v)) => received = Some(v),
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(Error::IntegrityFailed);
                }
                Err(mpsc::TryRecvError::Empty) => (),
            }
        }
        if let Some(status) = child.0.try_wait().map_err(|_| Error::IoFailed)? {
            if !status.success() {
                return Err(Error::IntegrityFailed);
            }
            if sent && received.is_some() {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let r = received.ok_or(Error::IntegrityFailed)?;
    let a = d.audio.as_ref().ok_or(Error::IntegrityFailed)?;
    let tail = r.tail_frame.as_ref().ok_or(Error::IntegrityFailed)?;
    if r.protocol_major != 1
        || r.consumer_instance_id != instance
        || r.descriptor_sha256 != nai_contracts::prepared_media_hash(d)
        || r.pcm_sha256 != a.pcm_sha256
        || r.sample_count != a.sample_count
        || r.expected_frames != d.expected_frames
        || r.first_frame_sha256 != d.first_frame_sha256
        || tail.frame_index != d.expected_frames
        || tail.sample_start != (d.expected_frames - 1) * 800
        || tail.sample_count as u64 != a.sample_count - tail.sample_start
        || tail.sample_rate != 24000
        || tail.width != 512
        || tail.height != 288
        || tail.pixels_sha256.len() != 64
    {
        return Err(Error::IntegrityFailed);
    }
    check()?;
    Ok(r)
}

pub(super) fn random_id() -> Result<String> {
    let mut random = [0u8; 16];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut random)
        .map_err(|_| Error::CapabilityUnavailable)?;
    random[6] = (random[6] & 15) | 64;
    random[8] = (random[8] & 63) | 128;
    let hex: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let instance = format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    );
    Ok(instance)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_probe_cannot_spawn_or_read_media() {
        let f = tempfile::tempfile().unwrap();
        let d = PreparedMediaDescriptor::default();
        assert_eq!(
            verify(f, &d, &|| Err(Error::Cancelled)),
            Err(Error::Cancelled)
        );
    }
}
