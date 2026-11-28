#![cfg(windows)]
//! Explicit real-device tests. All PCM is silent, HTTP service is a local fixture.
use nai_llm_openai::cosyvoice::Config;
#[path = "../../../tests/v561/cosyvoice_receipt_fixture.rs"]
mod completion_fixture;
use nai_workbench_engine::v561_stream_audio::{Options, audition};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    time::Duration,
};

fn binary() -> PathBuf {
    PathBuf::from(std::env::var_os("NAI_STREAM_AUDIO_OUTPUT").expect("set NAI_STREAM_AUDIO_OUTPUT"))
}
fn fixture(truncated: bool) -> (Config, std::thread::JoinHandle<()>, tempfile::TempDir) {
    let directory = tempfile::tempdir().unwrap();
    let reference = directory.path().join("ref.wav");
    std::fs::write(&reference, b"explicit fixture").unwrap();
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "http://{}/inference_zero_shot",
        server.local_addr().unwrap()
    );
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut header = vec![];
        let mut b = [0];
        while !header.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut b).unwrap();
            header.push(b[0]);
            assert!(header.len() < 16384);
        }
        let length = String::from_utf8(header)
            .unwrap()
            .lines()
            .find_map(|l| {
                l.split_once(':')
                    .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .map(|(_, v)| v.trim().parse::<usize>().unwrap())
            })
            .unwrap();
        let mut body = vec![0; length];
        stream.read_exact(&mut body).unwrap();
        stream.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\nTransfer-Encoding: chunked\r\nX-Sample-Rate: 24000\r\nX-Audio-Channels: 1\r\nX-Audio-Encoding: pcm_s16le\r\nX-Model-Stream: true\r\nX-Nai-Request: 11111111111111111111111111111111\r\nX-Nai-Instance: 22222222222222222222222222222222\r\n\r\n").unwrap();
        for i in 0..6 {
            if write!(stream, "2580\r\n")
                .and_then(|_| stream.write_all(&vec![0; 9600]))
                .and_then(|_| stream.write_all(b"\r\n"))
                .and_then(|_| stream.flush())
                .is_err()
            {
                return;
            }
            if truncated {
                std::thread::sleep(Duration::from_millis(150));
                return;
            }
            if i < 5 {
                std::thread::sleep(Duration::from_millis(300));
            }
        }
        let _ = stream.write_all(b"0\r\n\r\n");
        drop(stream);
        completion_fixture::serve(
            &server,
            28800,
            "c72274a22cba2dcef62e0f62834446ee39b0cd95ad0faa27a89b3b49f48aa390",
        );
    });
    (
        Config {
            endpoint: url,
            reference_file: reference,
            prompt_text: "fixture<|endofprompt|>".into(),
            deadline_ms: 10000,
            max_samples: 480000,
            lookahead: Default::default(),
        },
        thread,
        directory,
    )
}

#[test]
#[ignore = "explicit Windows real-device silent streaming"]
fn device_consumes_before_eof_and_survives_more_than_four_gaps() {
    let (config, thread, _directory) = fixture(false);
    let report = audition(config, "fixture".into(), &binary(), Options::default()).unwrap();
    thread.join().unwrap();
    assert_eq!(report.samples, 28800);
    assert_eq!(report.played_samples, 28800);
    assert!(report.first_played_ms < report.http_eof_ms);
}

#[test]
#[ignore = "explicit Windows real-device silent streaming"]
fn truncated_transport_cannot_report_completed() {
    let (config, thread, _directory) = fixture(true);
    assert!(audition(config, "fixture".into(), &binary(), Options::default()).is_err());
    thread.join().unwrap();
}

mod child {
    use super::*;
    use nai_contracts::v561_stream_audio::{
        Begin, Chunk, Command as Packet, Receipt, Renew, WireVersion, command::Action,
    };
    use prost::Message;
    use ring::{
        hmac,
        rand::{SecureRandom, SystemRandom},
    };
    use std::{
        os::windows::process::CommandExt,
        process::{Child, Command, Stdio},
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::windows::named_pipe::{ClientOptions, NamedPipeClient},
    };
    struct Reap(Child);
    impl Drop for Reap {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    struct Link {
        runtime: tokio::runtime::Runtime,
        pipe: NamedPipeClient,
        child: Reap,
        key: hmac::Key,
    }
    impl Link {
        fn start() -> Self {
            let mut random = [0; 32];
            SystemRandom::new().fill(&mut random).unwrap();
            let hex = |data: &[u8]| data.iter().map(|b| format!("{b:02x}")).collect::<String>();
            let name = format!(r"\\.\pipe\nai-v561-stream-{}", hex(&random));
            let token = [42; 32];
            let child = Reap(
                Command::new(binary())
                    .args(["--pipe", &name])
                    .env("NAI_V561_AUDIO_TOKEN", hex(&token))
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .creation_flags(0x08000000)
                    .spawn()
                    .unwrap(),
            );
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let pipe = runtime.block_on(async {
                let end = std::time::Instant::now() + Duration::from_secs(3);
                loop {
                    if let Ok(pipe) = ClientOptions::new().open(&name) {
                        break pipe;
                    }
                    assert!(std::time::Instant::now() < end);
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            });
            Self {
                runtime,
                pipe,
                child,
                key: hmac::Key::new(hmac::HMAC_SHA256, &token),
            }
        }
        fn packet(
            &mut self,
            sequence: u64,
            version: u32,
            epoch: u64,
            action: Action,
            corrupt: bool,
        ) {
            let mut packet = Packet {
                version,
                epoch,
                sequence,
                mac: vec![],
                action: Some(action),
            };
            packet.mac = hmac::sign(&self.key, &packet.encode_to_vec())
                .as_ref()
                .to_vec();
            if corrupt {
                packet.mac[0] ^= 1;
            }
            let bytes = packet.encode_to_vec();
            self.runtime.block_on(async {
                self.pipe.write_u32_le(bytes.len() as u32).await.unwrap();
                self.pipe.write_all(&bytes).await.unwrap();
                self.pipe.flush().await.unwrap();
            });
        }
        fn receipt(&mut self) -> Receipt {
            self.runtime.block_on(async {
                tokio::time::timeout(Duration::from_secs(3), async {
                    let n = self.pipe.read_u32_le().await.unwrap() as usize;
                    assert!(n < 1024);
                    let mut bytes = vec![0; n];
                    self.pipe.read_exact(&mut bytes).await.unwrap();
                    Receipt::decode(bytes.as_slice()).unwrap()
                })
                .await
                .unwrap()
            })
        }
        fn failed(&mut self) {
            let end = std::time::Instant::now() + Duration::from_secs(2);
            loop {
                if let Some(exit) = self.child.0.try_wait().unwrap() {
                    assert!(!exit.success());
                    break;
                }
                assert!(std::time::Instant::now() < end);
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    fn begin() -> Action {
        Action::Begin(Begin {
            sample_rate: 24000,
            capacity_samples: 96000,
            max_samples: 480000,
            prefill_samples: 2400,
        })
    }
    #[test]
    #[ignore = "explicit Windows continuous device stop and identity fault"]
    fn continuous_stop_and_unsealed_future_fail_closed() {
        use nai_contracts::v561_continuous_audio::{OpenScript, SegmentBegin, SegmentIdentity};
        for stop in [true, false] {
            let mut link = Link::start();
            link.packet(
                1,
                WireVersion::Current as u32,
                7,
                Action::OpenScript(OpenScript {
                    sample_rate: 24000,
                    capacity_samples: 96000,
                    max_samples: 24000,
                    prefill_samples: 2400,
                    max_sentences: 2,
                }),
                false,
            );
            link.receipt();
            if stop {
                link.packet(
                    2,
                    WireVersion::Current as u32,
                    7,
                    Action::Stop(Default::default()),
                    false,
                );
                assert_eq!(
                    link.receipt().state,
                    nai_contracts::v561_stream_audio::State::Stopped as i32
                );
            } else {
                for n in 1..=2 {
                    link.packet(
                        n as u64 + 1,
                        WireVersion::Current as u32,
                        7,
                        Action::SegmentBegin(SegmentBegin {
                            identity: Some(SegmentIdentity {
                                script_epoch: 7,
                                sentence_id: n,
                                generation_epoch: n as u64,
                                source_start_sample: 0,
                            }),
                            max_samples: 4800,
                        }),
                        false,
                    );
                    if n == 1 {
                        link.receipt();
                    }
                }
                link.failed();
            }
        }
    }
    #[test]
    #[ignore = "explicit real WASAPI device; silent three-segment continuity fixture"]
    fn continuous_three_segments_share_device_and_boundary_wait_excludes_source() {
        use nai_contracts::{
            v561_continuous_audio::{OpenScript, SegmentBegin, SegmentIdentity, SegmentSeal},
            v561_stream_audio::Seal,
        };
        use sha2::{Digest, Sha256};
        let mut link = Link::start();
        let mut sequence = 1;
        link.packet(
            sequence,
            WireVersion::Current as u32,
            7,
            Action::OpenScript(OpenScript {
                sample_rate: 24000,
                capacity_samples: 96000,
                max_samples: 24000,
                prefill_samples: 2400,
                max_sentences: 3,
            }),
            false,
        );
        let mut receipt = link.receipt();
        let generation = receipt.clock.unwrap().device_generation;
        let pid = link.child.0.id();
        let exchange = |link: &mut Link, sequence: &mut u64, action: Action| {
            *sequence += 1;
            link.packet(*sequence, WireVersion::Current as u32, 7, action, false);
            let r = link.receipt();
            let mut unsigned = r.clone();
            let mac = std::mem::take(&mut unsigned.mac);
            hmac::verify(&link.key, &unsigned.encode_to_vec(), &mac).unwrap();
            assert_eq!(r.sequence, *sequence);
            assert_eq!(r.epoch, 7);
            assert_eq!(r.clock.unwrap().device_generation, generation);
            assert!(r.clock_age_ms <= 200);
            r
        };
        let pcm = vec![0u8; 9600];
        let mut completed = 0;
        for n in 1..=3 {
            let identity = SegmentIdentity {
                script_epoch: 7,
                sentence_id: n,
                generation_epoch: n as u64,
                source_start_sample: (n as u64 - 1) * 4800,
            };
            exchange(
                &mut link,
                &mut sequence,
                Action::SegmentBegin(SegmentBegin {
                    identity: Some(identity),
                    max_samples: 4800,
                }),
            );
            let input = link.child.0.stdin.as_mut().unwrap();
            input.write_all(&(pcm.len() as u32).to_le_bytes()).unwrap();
            input.write_all(&pcm).unwrap();
            input.flush().unwrap();
            exchange(
                &mut link,
                &mut sequence,
                Action::Chunk(Chunk {
                    chunk_sequence: n as u64,
                    source_sample: identity.source_start_sample,
                    samples: 4800,
                    sha256: Sha256::digest(&pcm).to_vec(),
                }),
            );
            receipt = exchange(
                &mut link,
                &mut sequence,
                Action::SegmentSeal(SegmentSeal {
                    identity: Some(identity),
                    samples: 4800,
                    sha256: Sha256::digest(&pcm).to_vec(),
                }),
            );
            let end = n as u64 * 4800;
            let until = std::time::Instant::now() + Duration::from_secs(3);
            while receipt.played_samples < end {
                assert!(std::time::Instant::now() < until);
                receipt = exchange(
                    &mut link,
                    &mut sequence,
                    Action::Renew(Renew {
                        ttl_ms: 300,
                        play_until_sample: end,
                    }),
                );
                completed += receipt.segments.iter().filter(|s| s.completed).count();
                std::thread::sleep(Duration::from_millis(20));
            }
            for _ in 0..4 {
                receipt = exchange(
                    &mut link,
                    &mut sequence,
                    Action::Renew(Renew {
                        ttl_ms: 300,
                        play_until_sample: end,
                    }),
                );
                assert_eq!(receipt.played_samples, end); // real device silence is not source progress
                assert_eq!(link.child.0.id(), pid);
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        assert_eq!(completed, 3);
        receipt = exchange(
            &mut link,
            &mut sequence,
            Action::Seal(Seal {
                total_samples: 14400,
                sha256: Sha256::digest(vec![0u8; 28800]).to_vec(),
            }),
        );
        assert_eq!(
            receipt.state,
            nai_contracts::v561_stream_audio::State::Completed as i32
        );
        assert_eq!(receipt.played_samples, 14400);
        link.runtime.block_on(async {
            link.pipe.write_u8(0).await.unwrap();
        });
        println!(
            "continuous_device_verified pid={pid} generation={generation} segments={completed} played_samples=14400"
        );
    }
    #[test]
    #[ignore = "explicit Windows authenticated subprocess test"]
    fn forged_mac_version_and_epoch_are_rejected_before_device() {
        for (version, epoch, corrupt) in [
            (WireVersion::Current as u32, 1, true),
            (1, 1, false),
            (2, 1, false),
            (WireVersion::Current as u32, 0, false),
        ] {
            let mut link = Link::start();
            link.packet(1, version, epoch, begin(), corrupt);
            link.failed();
        }
    }
    #[test]
    #[ignore = "explicit Windows device lease and partial stdin test"]
    fn lease_loss_and_partial_pcm_cannot_hold_control_open() {
        for partial in [false, true] {
            let mut link = Link::start();
            link.packet(1, WireVersion::Current as u32, 1, begin(), false);
            assert_eq!(link.receipt().received_samples, 0);
            link.packet(
                2,
                WireVersion::Current as u32,
                1,
                Action::Renew(Renew {
                    ttl_ms: 50,
                    play_until_sample: 0,
                }),
                false,
            );
            link.receipt();
            if partial {
                let input = link.child.0.stdin.as_mut().unwrap();
                input.write_all(&9600u32.to_le_bytes()).unwrap();
                input.write_all(&[0]).unwrap();
                input.flush().unwrap();
                link.packet(
                    3,
                    WireVersion::Current as u32,
                    1,
                    Action::Chunk(Chunk {
                        chunk_sequence: 1,
                        source_sample: 0,
                        samples: 4800,
                        sha256: vec![0; 32],
                    }),
                    false,
                );
            }
            link.failed();
        }
    }
}
