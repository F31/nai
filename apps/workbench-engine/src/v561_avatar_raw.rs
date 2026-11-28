//! Private loopback raw sender, not yet installed in resident GPU startup.
//! Application-key mutual proof is not OS PID/ACL authentication.
use nai_contracts::{v561_avatar_stream::Chunk, workbench::WorkbenchError as Error};
use nai_runtime_core::foundation::Operation;
use prost::Message;
use ring::{
    hmac,
    rand::{SecureRandom, SystemRandom},
};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    time::Duration,
};

const CLIENT: &[u8] = b"nai.avatar.raw.v1\0";
const SERVER: &[u8] = b"nai.avatar.raw.server.v1\0";
pub struct Sender {
    listener: Option<TcpListener>,
    stream: Option<TcpStream>,
    key: zeroize::Zeroizing<[u8; 32]>,
    port: u16,
    pending: Option<u64>,
    last: u64,
}
fn write(stream: &mut TcpStream, mut data: &[u8], op: &Operation) -> Result<(), Error> {
    while !data.is_empty() {
        op.check()?;
        match stream.write(data) {
            Ok(0) => return Err(Error::IoFailed),
            Ok(n) => data = &data[n..],
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(_) => return Err(Error::IoFailed),
        }
    }
    op.check()
}
fn read(stream: &mut TcpStream, mut data: &mut [u8], op: &Operation) -> Result<(), Error> {
    while !data.is_empty() {
        op.check()?;
        match stream.read(data) {
            Ok(0) => return Err(Error::IoFailed),
            Ok(n) => data = &mut data[n..],
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(_) => return Err(Error::IoFailed),
        }
    }
    op.check()
}
impl Sender {
    pub fn bind(op: &Operation) -> Result<Self, Error> {
        op.check()?;
        let listener =
            TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).map_err(|_| Error::IoFailed)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| Error::IoFailed)?;
        let port = listener.local_addr().map_err(|_| Error::IoFailed)?.port();
        let mut key = zeroize::Zeroizing::new([0; 32]);
        SystemRandom::new()
            .fill(&mut *key)
            .map_err(|_| Error::IoFailed)?;
        Ok(Self {
            listener: Some(listener),
            stream: None,
            key,
            port,
            pending: None,
            last: 0,
        })
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    /// Supply only to the intended spawned child's private environment.
    pub fn key(&self) -> &[u8] {
        &*self.key
    }
    pub fn accept(&mut self, parent: &Operation) -> Result<(), Error> {
        let result = (|| {
            let op = parent.child(4000)?;
            let listener = self.listener.take().ok_or(Error::ProtocolError)?;
            let mut stream = loop {
                op.check()?;
                match listener.accept() {
                    Ok((s, peer)) if peer.ip().is_loopback() => break s,
                    Ok(_) => return Err(Error::PermissionDenied),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => return Err(Error::IoFailed),
                }
            };
            stream.set_nonblocking(true).map_err(|_| Error::IoFailed)?;
            let key = hmac::Key::new(hmac::HMAC_SHA256, self.key());
            let mut nonce = [0; 32];
            SystemRandom::new()
                .fill(&mut nonce)
                .map_err(|_| Error::IoFailed)?;
            write(&mut stream, &nonce, &op)?;
            let mut proof = [0; 32];
            read(&mut stream, &mut proof, &op)?;
            let bytes = [CLIENT, nonce.as_slice()].concat();
            hmac::verify(&key, &bytes, &proof).map_err(|_| Error::PermissionDenied)?;
            let confirmation =
                hmac::sign(&key, &[SERVER, nonce.as_slice(), proof.as_slice()].concat());
            write(&mut stream, confirmation.as_ref(), &op)?;
            self.stream = Some(stream);
            Ok(())
        })();
        if result.is_err() {
            self.close();
            parent.cancel();
        }
        result
    }
    pub fn send(&mut self, chunk: &Chunk, pcm: &[u8], parent: &Operation) -> Result<(), Error> {
        let result = (|| {
            let op = parent.child(4000)?;
            if self.pending.is_some()
                || chunk.raw_payload_id <= self.last
                || !(1..=38400).contains(&chunk.samples)
                || pcm.len() != chunk.samples as usize * 2
                || Sha256::digest(pcm).as_slice() != chunk.sha256
            {
                return Err(Error::ProtocolError);
            }
            let header = chunk.encode_to_vec();
            if header.is_empty() || header.len() > 256 {
                return Err(Error::ResourceExhausted);
            }
            let stream = self.stream.as_mut().ok_or(Error::CapabilityUnavailable)?;
            write(stream, &(header.len() as u32).to_le_bytes(), &op)?;
            write(stream, &header, &op)?;
            write(stream, pcm, &op)?;
            self.pending = Some(chunk.raw_payload_id);
            self.last = chunk.raw_payload_id;
            Ok(())
        })();
        if result.is_err() {
            self.close();
            parent.cancel();
        }
        result
    }
    /// Caller must first authenticate the matching CHUNK_CONSUMED control event.
    pub fn consumed(&mut self, id: u64) -> Result<(), Error> {
        if self.pending != Some(id) {
            self.close();
            return Err(Error::ProtocolError);
        }
        self.pending = None;
        Ok(())
    }
    /// Only after authenticated RENDER_CANCELLED/CANCELLED confirms the receiver
    /// kept framing. This retires a fully written block without pretending EOS.
    pub(crate) fn cancelled(&mut self) -> Result<(), Error> {
        if self.stream.is_none() {
            return Err(Error::CapabilityUnavailable);
        }
        self.pending = None;
        Ok(())
    }
    pub fn close(&mut self) {
        self.stream.take();
        self.listener.take();
        self.pending = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nai_contracts::v561_avatar_stream::{self as wire, request::Action};
    use std::{
        process::{Child, Command, Stdio},
        sync::mpsc,
        time::Instant,
    };
    struct Probe {
        child: Child,
        events: mpsc::Receiver<wire::Event>,
        reader: Option<std::thread::JoinHandle<()>>,
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            if let Some(t) = self.reader.take() {
                let _ = t.join();
            }
        }
    }
    fn spawn(sender: &Sender) -> Probe {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let python =
            std::env::var_os("NAI_V561_PYTHON").expect("set Python for native transport test");
        let mut child = Command::new(python)
            .arg("-B")
            .arg(root.join("tests/v561/avatar_stream_pipe_child.py"))
            .arg(sender.port().to_string())
            .env(
                "NAI_AVATAR_RAW_FIXTURE_KEY",
                sender
                    .key()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::sync_channel(8);
        let reader = std::thread::spawn(move || {
            loop {
                let mut prefix = [0; 4];
                if stdout.read_exact(&mut prefix).is_err() {
                    break;
                }
                let size = u32::from_le_bytes(prefix) as usize;
                if !(1..=16384).contains(&size) {
                    break;
                }
                let mut bytes = vec![0; size];
                if stdout.read_exact(&mut bytes).is_err() {
                    break;
                }
                let Ok(event) = wire::Event::decode(bytes.as_slice()) else {
                    break;
                };
                if tx.send(event).is_err() {
                    break;
                }
            }
        });
        Probe {
            child,
            events: rx,
            reader: Some(reader),
        }
    }
    fn control(probe: &mut Probe, key: &[u8], seq: u64, action: Action) {
        let mut r = wire::Request {
            version: wire::WireVersion::Current as u32,
            sequence: seq,
            task_id: 5,
            epoch: 6,
            mac: vec![],
            action: Some(action),
        };
        r.mac = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, key), &r.encode_to_vec())
            .as_ref()
            .to_vec();
        let bytes = r.encode_to_vec();
        let stdin = probe.child.stdin.as_mut().unwrap();
        stdin
            .write_all(&(bytes.len() as u32).to_le_bytes())
            .unwrap();
        stdin.write_all(&bytes).unwrap();
        stdin.flush().unwrap();
    }
    fn event(probe: &Probe, key: &[u8], kind: wire::Kind) -> wire::Event {
        let mut e = probe
            .events
            .recv_timeout(Duration::from_secs(6))
            .expect("bounded signed child event");
        let mac = std::mem::take(&mut e.mac);
        hmac::verify(
            &hmac::Key::new(hmac::HMAC_SHA256, key),
            &e.encode_to_vec(),
            &mac,
        )
        .unwrap();
        assert_eq!(
            (e.version, e.task_id, e.epoch),
            (wire::WireVersion::Current as u32, 5, 6)
        );
        assert_eq!(e.kind, kind as i32);
        e
    }
    fn begin(probe: &mut Probe, key: &[u8]) {
        control(
            probe,
            key,
            1,
            Action::Begin(wire::Begin {
                portrait_sha256: vec![8; 32],
                block_frames: 40,
                sample_rate: 24000,
                max_samples: 144000,
            }),
        );
        event(probe, key, wire::Kind::InputAccepted);
    }
    fn finish(probe: &mut Probe) {
        probe.child.stdin.take();
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(status) = probe.child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < until, "child failed bounded cleanup");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn native_separate_control_pcm_consumption_and_eos() {
        let op = Operation::model_task(15000).unwrap();
        let mut sender = Sender::bind(&op).unwrap();
        let key = sender.key().to_vec();
        let mut probe = spawn(&sender);
        sender.accept(&op).unwrap();
        begin(&mut probe, &key);
        let pcm = (0..117120u32)
            .flat_map(|v| (v as i16).to_le_bytes())
            .collect::<Vec<_>>();
        let mut seq = 2;
        for (i, part) in pcm.chunks(76800).enumerate() {
            let chunk = wire::Chunk {
                raw_payload_id: i as u64 + 1,
                start_sample: (i * 38400) as u64,
                samples: (part.len() / 2) as u32,
                sha256: Sha256::digest(part).to_vec(),
            };
            control(&mut probe, &key, seq, Action::Chunk(chunk.clone()));
            sender.send(&chunk, part, &op).unwrap();
            let ack = event(&probe, &key, wire::Kind::ChunkConsumed);
            assert_eq!(
                (ack.request_sequence, ack.raw_payload_id),
                (seq, chunk.raw_payload_id)
            );
            sender.consumed(ack.raw_payload_id).unwrap();
            seq += 1;
        }
        control(
            &mut probe,
            &key,
            seq,
            Action::Eos(wire::Eos {
                samples: 117120,
                sha256: Sha256::digest(&pcm).to_vec(),
                provider_eos_verified: true,
            }),
        );
        let eos = event(&probe, &key, wire::Kind::EosVerified);
        assert_eq!(eos.samples, 117120);
        assert_eq!(eos.sha256, Sha256::digest(&pcm).to_vec());
        finish(&mut probe);
    }
    #[test]
    fn native_partial_raw_packet_does_not_block_control_cancel() {
        let op = Operation::model_task(15000).unwrap();
        let mut sender = Sender::bind(&op).unwrap();
        let key = sender.key().to_vec();
        let mut probe = spawn(&sender);
        sender.accept(&op).unwrap();
        begin(&mut probe, &key);
        let chunk = wire::Chunk {
            raw_payload_id: 1,
            start_sample: 0,
            samples: 2,
            sha256: Sha256::digest([1u8, 0, 2, 0]).to_vec(),
        };
        control(&mut probe, &key, 2, Action::Chunk(chunk.clone()));
        let bytes = chunk.encode_to_vec();
        let stream = sender.stream.as_mut().unwrap();
        write(stream, &(bytes.len() as u32).to_le_bytes(), &op).unwrap();
        write(stream, &bytes, &op).unwrap();
        write(stream, &[1], &op).unwrap();
        let started = Instant::now();
        control(&mut probe, &key, 3, Action::Cancel(wire::Cancel {}));
        let ack = event(&probe, &key, wire::Kind::Cancelled);
        assert_eq!(ack.samples, 0);
        assert!(started.elapsed() < Duration::from_millis(500));
        finish(&mut probe);
    }
    #[test]
    fn native_wrong_peer_key_is_rejected_and_cancels_parent() {
        let op = Operation::model_task(4000).unwrap();
        let mut sender = Sender::bind(&op).unwrap();
        let port = sender.port();
        let client = std::thread::spawn(move || {
            let mut s = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut nonce = [0; 32];
            s.read_exact(&mut nonce).unwrap();
            s.write_all(&[0; 32]).unwrap();
        });
        assert_eq!(sender.accept(&op), Err(Error::PermissionDenied));
        assert_eq!(op.check(), Err(Error::Cancelled));
        assert!(sender.stream.is_none() && sender.listener.is_none());
        client.join().unwrap();
    }
}
