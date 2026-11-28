use nai_contracts::v561_avatar_stream::{self as wire, request::Action};
use prost::Message;
use sha2::{Digest, Sha256};

// Test-only HMAC oracle with a public fixed fixture key; no production secrets.
fn mac(bytes: &[u8]) -> Vec<u8> {
    let mut inner = Sha256::new();
    inner.update([7u8 ^ 0x36; 32]);
    inner.update([0x36; 32]);
    inner.update(bytes);
    let mut outer = Sha256::new();
    outer.update([7u8 ^ 0x5c; 32]);
    outer.update([0x5c; 32]);
    outer.update(inner.finalize());
    outer.finalize().to_vec()
}

#[test]
fn rust_signed_inputs_python_consumption_rust_eos() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("target-windows/v561-avatar-stream-contract");
    std::fs::create_dir_all(&dir).unwrap();
    let pcm = [1u8, 0, 2, 0];
    let digest = Sha256::digest(pcm).to_vec();
    let actions = [
        Action::Begin(wire::Begin {
            portrait_sha256: vec![8; 32],
            block_frames: 40,
            sample_rate: 24000,
            max_samples: 144000,
        }),
        Action::Chunk(wire::Chunk {
            raw_payload_id: 1,
            start_sample: 0,
            samples: 2,
            sha256: digest.clone(),
        }),
        Action::Eos(wire::Eos {
            samples: 2,
            sha256: digest.clone(),
            provider_eos_verified: true,
        }),
    ];
    let mut bytes = Vec::new();
    for (index, action) in actions.into_iter().enumerate() {
        let mut request = wire::Request {
            version: wire::WireVersion::Current as u32,
            sequence: index as u64 + 1,
            task_id: 5,
            epoch: 6,
            mac: vec![],
            action: Some(action),
        };
        request.mac = mac(&request.encode_to_vec());
        let encoded = request.encode_to_vec();
        bytes.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&encoded);
    }
    let source = dir.join("rust-requests.pb");
    let response = dir.join("python-eos.pb");
    std::fs::write(&source, bytes).unwrap();
    let python = std::env::var_os("NAI_V561_PYTHON")
        .expect("set NAI_V561_PYTHON for actual cross-language test");
    assert!(
        std::process::Command::new(python)
            .arg("-B")
            .arg(root.join("tests/v561/avatar_stream_roundtrip.py"))
            .arg(source)
            .arg(&response)
            .status()
            .unwrap()
            .success()
    );
    let mut event = wire::Event::decode(std::fs::read(response).unwrap().as_slice()).unwrap();
    let signed = std::mem::take(&mut event.mac);
    assert_eq!(signed, mac(&event.encode_to_vec()));
    assert_eq!(
        (
            event.task_id,
            event.epoch,
            event.request_sequence,
            event.samples
        ),
        (5, 6, 3, 2)
    );
    assert_eq!(event.kind, wire::Kind::EosVerified as i32);
    assert_eq!(event.sha256, digest);
}
