use nai_contracts::workbench::WorkbenchError as Error;
use nai_runtime_core::{
    foundation::{Result, uuid},
    llm::CredentialVault,
};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use serde_json::{Value, json};
use zeroize::Zeroizing;
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(s: &str) -> Result<Vec<u8>> {
    if !s.len().is_multiple_of(2) || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::IntegrityFailed);
    }
    s.as_bytes()
        .chunks_exact(2)
        .map(|c| {
            u8::from_str_radix(
                std::str::from_utf8(c).map_err(|_| Error::IntegrityFailed)?,
                16,
            )
            .map_err(|_| Error::IntegrityFailed)
        })
        .collect()
}
pub struct NativeVault {
    scope: String,
    speech: bool,
}
impl NativeVault {
    pub fn new(scope: String) -> Self {
        Self {
            scope,
            speech: false,
        }
    }
    pub fn for_speech(scope: String) -> Self {
        Self {
            scope,
            speech: true,
        }
    }
}
fn aad(ws: &str, id: &str, endpoint: &str, speech: bool) -> Vec<u8> {
    serde_json::json!([
        if speech {
            "nai-workbench-v43-tts"
        } else {
            "nai-workbench-v43-llm"
        },
        ws,
        id,
        endpoint
    ])
    .to_string()
    .into_bytes()
}
impl NativeVault {
    #[cfg(windows)]
    fn entry(&self, ws: &str, reference: &str) -> Result<keyring::Entry> {
        uuid(ws)?;
        uuid(reference)?;
        keyring::Entry::new(
            if self.speech {
                "nai.workbench.v43.tts"
            } else {
                "nai.workbench.v43.llm"
            },
            &format!("{}:{ws}:{reference}", self.scope),
        )
        .map_err(|_| Error::CapabilityUnavailable)
    }
    fn load(&self, ws: &str, reference: &str) -> Result<Zeroizing<Vec<u8>>> {
        #[cfg(windows)]
        {
            self.entry(ws, reference)?
                .get_secret()
                .map(Zeroizing::new)
                .map_err(|_| Error::CapabilityUnavailable)
        }
        #[cfg(not(windows))]
        {
            let _ = (&self.scope, ws, reference);
            Err(Error::CapabilityUnavailable)
        }
    }
    fn store(&self, ws: &str, reference: &str, key: &[u8]) -> Result<()> {
        #[cfg(windows)]
        {
            self.entry(ws, reference)?
                .set_secret(key)
                .map_err(|_| Error::CapabilityUnavailable)
        }
        #[cfg(not(windows))]
        {
            let _ = (&self.scope, ws, reference, key);
            Err(Error::CapabilityUnavailable)
        }
    }
}
impl CredentialVault for NativeVault {
    fn seal(&self, ws: &str, id: &str, endpoint: &str, secret: &[u8]) -> Result<Value> {
        uuid(ws)?;
        uuid(id)?;
        if secret.is_empty() || secret.len() > 4096 || secret.iter().any(|b| *b < 33 || *b > 126) {
            return Err(Error::InvalidArgument);
        }
        let rng = SystemRandom::new();
        let mut raw = Zeroizing::new(vec![0; 32]);
        let mut nonce = [0; 12];
        let mut reference = [0; 16];
        rng.fill(&mut raw)
            .map_err(|_| Error::CapabilityUnavailable)?;
        rng.fill(&mut nonce)
            .map_err(|_| Error::CapabilityUnavailable)?;
        rng.fill(&mut reference)
            .map_err(|_| Error::CapabilityUnavailable)?;
        let h = hex(&reference);
        let reference = format!(
            "{}-{}-{}-{}-{}",
            &h[..8],
            &h[8..12],
            &h[12..16],
            &h[16..20],
            &h[20..]
        );
        let key = aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::AES_256_GCM, &raw).map_err(|_| Error::IntegrityFailed)?,
        );
        let mut sealed = secret.to_vec();
        key.seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(aad(ws, id, endpoint, self.speech)),
            &mut sealed,
        )
        .map_err(|_| Error::IntegrityFailed)?;
        self.store(ws, &reference, &raw)?;
        Ok(json!({"reference":reference,"nonce":hex(&nonce),"ciphertext":hex(&sealed)}))
    }
    fn open(
        &self,
        ws: &str,
        id: &str,
        endpoint: &str,
        sealed: &Value,
    ) -> Result<Zeroizing<Vec<u8>>> {
        uuid(ws)?;
        uuid(id)?;
        let reference = sealed["reference"].as_str().ok_or(Error::IntegrityFailed)?;
        let raw = self.load(ws, reference)?;
        let nonce = unhex(sealed["nonce"].as_str().ok_or(Error::IntegrityFailed)?)?;
        let nonce: [u8; 12] = nonce.try_into().map_err(|_| Error::IntegrityFailed)?;
        let mut bytes = Zeroizing::new(unhex(
            sealed["ciphertext"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?,
        )?);
        let key = aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::AES_256_GCM, &raw).map_err(|_| Error::IntegrityFailed)?,
        );
        let plain = key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(aad(ws, id, endpoint, self.speech)),
                &mut bytes,
            )
            .map_err(|_| Error::IntegrityFailed)?
            .len();
        bytes.truncate(plain);
        Ok(bytes)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(not(windows))]
    fn native_credentials_are_unavailable_not_plaintext_files() {
        let v = NativeVault::new("test-only".into());
        assert!(matches!(
            v.seal(
                "00000000-0000-0000-0000-000000000001",
                "00000000-0000-0000-0000-000000000002",
                "https://example.invalid",
                b"test-key"
            ),
            Err(Error::CapabilityUnavailable)
        ));
    }
    #[test]
    #[cfg(windows)]
    fn speech_credentials_use_an_independent_service_and_aad_domain() {
        let ws = "00000000-0000-0000-0000-000000000001";
        let id = "00000000-0000-0000-0000-000000000002";
        let scope = format!(
            "speech-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let speech = NativeVault::for_speech(scope.clone());
        let text = NativeVault::new(scope);
        let sealed = speech
            .seal(ws, id, "https://example.invalid", b"synthetic-speech-key")
            .unwrap();
        assert_eq!(
            &**speech
                .open(ws, id, "https://example.invalid", &sealed)
                .unwrap(),
            b"synthetic-speech-key"
        );
        assert!(
            text.open(ws, id, "https://example.invalid", &sealed)
                .is_err()
        );
        assert_ne!(
            aad(ws, id, "https://example.invalid", true),
            aad(ws, id, "https://example.invalid", false)
        );
        speech
            .entry(ws, sealed["reference"].as_str().unwrap())
            .unwrap()
            .delete_credential()
            .unwrap();
    }
    #[test]
    #[cfg(windows)]
    fn native_shared_catalog_rewrap_preserves_source_for_text_and_speech() {
        use nai_runtime_core::{
            credential_rewrap::{Identity, rewrap},
            foundation::Operation,
        };
        let ws = "00000000-0000-0000-0000-000000000001";
        let old = "00000000-0000-0000-0000-000000000002";
        let shared = "00000000-0000-4000-8000-000000004802";
        let new = "00000000-0000-0000-0000-000000000003";
        let url = "https://example.invalid/v1";
        for speech in [false, true] {
            let scope = format!(
                "test-rewrap-{}-{}-{speech}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            );
            let v = if speech {
                NativeVault::for_speech(scope)
            } else {
                NativeVault::new(scope)
            };
            let source = v.seal(ws, old, url, b"synthetic-rewrap-key").unwrap();
            let target = rewrap(
                &Operation::new(4000).unwrap(),
                &v,
                &v,
                Identity {
                    workspace: ws,
                    model: old,
                    endpoint: url,
                },
                Identity {
                    workspace: shared,
                    model: new,
                    endpoint: url,
                },
                &source,
            )
            .unwrap();
            let original = v.open(ws, old, url, &source);
            let migrated = v.open(shared, new, url, &target);
            let wrong = v.open(ws, new, url, &target);
            // Only entries created by this test are removed, before assertions.
            v.entry(ws, source["reference"].as_str().unwrap())
                .unwrap()
                .delete_credential()
                .unwrap();
            v.entry(shared, target["reference"].as_str().unwrap())
                .unwrap()
                .delete_credential()
                .unwrap();
            assert_eq!(&**original.unwrap(), b"synthetic-rewrap-key");
            assert_eq!(&**migrated.unwrap(), b"synthetic-rewrap-key");
            assert!(wrong.is_err());
            assert!(!target.to_string().contains("synthetic-rewrap-key"));
        }
    }
    #[test]
    #[cfg(windows)]
    fn native_cipher_roundtrip_scope_tamper_and_cleanup() {
        let ws = "00000000-0000-0000-0000-000000000001";
        let id = "00000000-0000-0000-0000-000000000002";
        let scope = format!(
            "test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let v = NativeVault::new(scope);
        let s = v
            .seal(ws, id, "https://example.invalid", b"synthetic-test-key")
            .unwrap();
        {
            assert!(!s.to_string().contains("synthetic-test-key"));
            assert_eq!(
                &**v.open(ws, id, "https://example.invalid", &s).unwrap(),
                b"synthetic-test-key"
            );
            assert!(v.open(ws, id, "https://other.invalid", &s).is_err());
            let mut changed = s.clone();
            changed["ciphertext"] = "00".repeat(32).into();
            assert!(v.open(ws, id, "https://example.invalid", &changed).is_err());
        }
        v.entry(ws, s["reference"].as_str().unwrap())
            .unwrap()
            .delete_credential()
            .unwrap();
    }
}
