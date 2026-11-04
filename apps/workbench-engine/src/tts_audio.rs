//! Private, ephemeral PCM artifacts. No files, devices, control payloads or API calls.
use nai_contracts::workbench::{TtsAudioAuthorization, WorkbenchError as Error};
use nai_runtime_core::{
    foundation::uuid,
    tts::{MAX_SAMPLES, SAMPLE_RATE},
};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;
struct Artifact {
    ws: String,
    id: String,
    task_sha: String,
    sha: String,
    samples: Zeroizing<Vec<i16>>,
    until: Instant,
    lease: Option<(u64, Instant)>,
}
pub struct Cache {
    fence: std::sync::Arc<nai_runtime_core::foundation::StopFence>,
    items: Vec<Artifact>,
    epoch: u64,
}
pub(super) struct PreparationSamples<'a> {
    pub audio: nai_contracts::workbench::PublicMediaAudio,
    pub samples: &'a [i16],
    pub until: Instant,
}
impl Cache {
    pub(super) fn preparation_samples(
        &self,
        ws: &str,
        id: &str,
        task_sha: &str,
        now: Instant,
        op: &nai_runtime_core::foundation::Operation,
    ) -> Result<PreparationSamples<'_>, Error> {
        let audio = self.observe(ws, id, task_sha, now, op)?;
        let a = self
            .items
            .iter()
            .find(|a| a.ws == ws && a.id == id)
            .ok_or(Error::CapabilityUnavailable)?;
        Ok(PreparationSamples {
            audio,
            samples: &a.samples,
            until: a.until,
        })
    }
    /// Read-only observation for candidate preparation. No audition or public lease is granted.
    pub fn observe(
        &self,
        ws: &str,
        id: &str,
        task_sha: &str,
        now: Instant,
        op: &nai_runtime_core::foundation::Operation,
    ) -> Result<nai_contracts::workbench::PublicMediaAudio, Error> {
        let generation = self.fence.generation();
        self.fence.check(generation)?;
        op.check()?;
        let started = Instant::now();
        let a = self
            .items
            .iter()
            .find(|a| a.ws == ws && a.id == id)
            .ok_or(Error::CapabilityUnavailable)?;
        if now >= a.until {
            return Err(Error::DeadlineExceeded);
        }
        if a.task_sha != task_sha {
            return Err(Error::RevisionConflict);
        }
        let sha = nai_media_core::public_clip::pcm_hash(&a.samples, &|| {
            op.check()?;
            self.fence.check(generation)
        })?;
        if sha != a.sha {
            return Err(Error::IntegrityFailed);
        }
        self.fence.check(generation)?;
        op.check()?;
        if now
            .checked_add(started.elapsed())
            .ok_or(Error::DeadlineExceeded)?
            >= a.until
        {
            return Err(Error::DeadlineExceeded);
        }
        Ok(nai_contracts::workbench::PublicMediaAudio {
            artifact_id: a.id.clone(),
            pcm_sha256: sha,
            sample_count: a.samples.len() as u64,
            sample_rate: SAMPLE_RATE,
            channels: 1,
        })
    }
    pub fn new(seed: u64, fence: std::sync::Arc<nai_runtime_core::foundation::StopFence>) -> Self {
        Self {
            items: Vec::new(),
            epoch: seed,
            fence,
        }
    }
    pub fn purge(&mut self, now: Instant) {
        self.items.retain(|a| a.until > now);
    }
    pub fn available(&mut self, now: Instant) -> Result<(), Error> {
        self.fence.check(self.fence.generation())?;
        self.purge(now);
        if self.items.len() >= 2 {
            Err(Error::ResourceExhausted)
        } else {
            Ok(())
        }
    }
    pub fn insert(
        &mut self,
        ws: &str,
        id: &str,
        task_sha: &str,
        expected_sha: &str,
        samples: Zeroizing<Vec<i16>>,
        now: Instant,
    ) -> Result<(), Error> {
        uuid(ws)?;
        uuid(id)?;
        self.available(now)?;
        if samples.is_empty()
            || samples.len() > MAX_SAMPLES
            || task_sha.len() != 64
            || !task_sha.bytes().all(|b| b.is_ascii_hexdigit())
            || self.items.iter().any(|a| a.id == id)
        {
            return Err(Error::InvalidArgument);
        }
        let mut h = Sha256::new();
        for n in samples.iter() {
            h.update(n.to_le_bytes());
        }
        let sha: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if sha != expected_sha {
            return Err(Error::IntegrityFailed);
        }
        self.items.push(Artifact {
            ws: ws.into(),
            id: id.into(),
            task_sha: task_sha.into(),
            sha,
            samples,
            until: now + Duration::from_secs(60),
            lease: None,
        });
        Ok(())
    }
    // Explicit object/hash/time bindings are kept together at this gate.
    #[allow(clippy::too_many_arguments)]
    pub fn authorize(
        &mut self,
        ws: &str,
        id: &str,
        task_sha: &str,
        pcm_sha: &str,
        count: u64,
        now: Instant,
        ttl_ms: u32,
    ) -> Result<TtsAudioAuthorization, Error> {
        self.purge(now);
        if ttl_ms == 0 || ttl_ms > 500 {
            return Err(Error::InvalidArgument);
        }
        let a = self
            .items
            .iter_mut()
            .find(|a| a.ws == ws && a.id == id)
            .ok_or(Error::CapabilityUnavailable)?;
        if a.task_sha != task_sha || a.sha != pcm_sha || a.samples.len() as u64 != count {
            return Err(Error::RevisionConflict);
        }
        self.epoch = self.epoch.checked_add(1).ok_or(Error::ResourceExhausted)?;
        let ttl = (a.until - now).as_millis().min(ttl_ms.into()) as u32;
        if ttl == 0 {
            return Err(Error::DeadlineExceeded);
        }
        a.lease = Some((self.epoch, now + Duration::from_millis(ttl.into())));
        let p = TtsAudioAuthorization {
            artifact_id: a.id.clone(),
            sample_count: count,
            sample_rate: SAMPLE_RATE,
            pcm_sha256: a.sha.clone(),
            lease_epoch: self.epoch,
            ttl_ms: ttl,
        };
        self.read(ws, &p, now)?;
        Ok(p)
    }
    pub(super) fn read(
        &self,
        ws: &str,
        p: &TtsAudioAuthorization,
        now: Instant,
    ) -> Result<&[i16], Error> {
        self.fence.check(self.fence.generation())?;
        let a = self
            .items
            .iter()
            .find(|a| a.ws == ws && a.id == p.artifact_id)
            .ok_or(Error::CapabilityUnavailable)?;
        let (epoch, until) = a.lease.ok_or(Error::PermissionDenied)?;
        if now >= until || now >= a.until {
            return Err(Error::DeadlineExceeded);
        }
        if epoch != p.lease_epoch
            || a.sha != p.pcm_sha256
            || a.samples.len() as u64 != p.sample_count
            || p.sample_rate != SAMPLE_RATE
        {
            return Err(Error::RevisionConflict);
        }
        Ok(&a.samples)
    }
    pub fn revoke(&mut self, ws: &str, id: &str) {
        self.items.retain(|a| a.ws != ws || a.id != id);
    }
    pub fn references(&self) -> Vec<(String, String)> {
        self.items
            .iter()
            .map(|a| (a.ws.clone(), a.id.clone()))
            .collect()
    }
    pub fn clear(&mut self) {
        self.items.clear();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const WS: &str = "00000000-0000-0000-0000-000000000001";
    const ID: &str = "00000000-0000-0000-0000-000000000002";
    #[test]
    fn public_observation_rehashes_pcm_without_creating_or_renewing_any_lease() {
        let now = Instant::now();
        let mut c = Cache::new(1, std::sync::Arc::default());
        insert(&mut c, ID, now);
        let before = c.items[0].until;
        let op = nai_runtime_core::foundation::Operation::new(4000).unwrap();
        let observed = c.observe(WS, ID, &"aa".repeat(32), now, &op).unwrap();
        assert_eq!(observed.sample_count, 1);
        assert_eq!(observed.pcm_sha256, c.items[0].sha);
        assert_eq!(c.items[0].until, before);
        assert!(c.items[0].lease.is_none());
        assert_eq!(
            c.observe(WS, ID, &"bb".repeat(32), now, &op),
            Err(Error::RevisionConflict)
        );
        c.items[0].samples[0] = 2;
        assert_eq!(
            c.observe(WS, ID, &"aa".repeat(32), now, &op),
            Err(Error::IntegrityFailed)
        );
    }
    #[test]
    fn public_observation_rejects_expiry_release_cancel_and_p0() {
        for failure in 0..4 {
            let now = Instant::now();
            let mut c = Cache::new(1, std::sync::Arc::default());
            insert(&mut c, ID, now);
            let op = nai_runtime_core::foundation::Operation::new(4000).unwrap();
            let mut tick = now;
            match failure {
                0 => tick += Duration::from_secs(60),
                1 => c.revoke(WS, ID),
                2 => op.cancel(),
                _ => c.fence.stop(),
            }
            assert!(c.observe(WS, ID, &"aa".repeat(32), tick, &op).is_err());
        }
    }
    fn insert(c: &mut Cache, id: &str, t: Instant) {
        let mut h = Sha256::new();
        h.update(1i16.to_le_bytes());
        let sha: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
        c.insert(WS, id, &"aa".repeat(32), &sha, Zeroizing::new(vec![1]), t)
            .unwrap();
    }
    fn permit(c: &mut Cache, id: &str, t: Instant) -> TtsAudioAuthorization {
        let a = c.items.iter().find(|a| a.id == id).unwrap();
        let sha = a.sha.clone();
        c.authorize(WS, id, &"aa".repeat(32), &sha, 1, t, 500)
            .unwrap()
    }
    #[test]
    fn capacity_expiry_and_identity_are_enforced() {
        let now = Instant::now();
        let mut c = Cache::new(1, std::sync::Arc::default());
        insert(&mut c, ID, now);
        insert(&mut c, "00000000-0000-0000-0000-000000000003", now);
        assert_eq!(c.available(now), Err(Error::ResourceExhausted));
        let p = permit(&mut c, ID, now);
        assert_eq!(c.read(WS, &p, now).unwrap(), &[1]);
        assert!(c.read("wrong", &p, now).is_err());
        let mut bad = p.clone();
        bad.pcm_sha256 = "bb".repeat(32);
        assert!(c.read(WS, &bad, now).is_err());
        assert!(c.read(WS, &p, now + Duration::from_millis(500)).is_err());
        assert!(c.available(now + Duration::from_secs(60)).is_ok());
        assert!(c.read(WS, &p, now + Duration::from_secs(60)).is_err());
    }
    #[test]
    fn reauthorization_revocation_and_new_owner_never_restore_old_lease() {
        let now = Instant::now();
        let mut c = Cache::new(10, std::sync::Arc::default());
        insert(&mut c, ID, now);
        let old = permit(&mut c, ID, now);
        let latest = permit(&mut c, ID, now);
        assert!(c.read(WS, &old, now).is_err());
        c.revoke(WS, ID);
        assert!(c.read(WS, &latest, now).is_err());
        let mut fresh = Cache::new(100, std::sync::Arc::default());
        insert(&mut fresh, ID, now);
        let _ = permit(&mut fresh, ID, now);
        assert!(fresh.read(WS, &latest, now).is_err());
        fresh.clear();
        assert!(fresh.items.is_empty());
    }
    #[test]
    fn corrupt_audio_and_stale_task_never_get_authorization() {
        let now = Instant::now();
        let mut c = Cache::new(1, std::sync::Arc::default());
        assert!(
            c.insert(
                WS,
                ID,
                &"aa".repeat(32),
                &"bb".repeat(32),
                Zeroizing::new(vec![1]),
                now
            )
            .is_err()
        );
        insert(&mut c, ID, now);
        let sha = c.items[0].sha.clone();
        assert_eq!(
            c.authorize(WS, ID, &"bb".repeat(32), &sha, 1, now, 500),
            Err(Error::RevisionConflict)
        );
        assert!(
            c.authorize(WS, ID, &"aa".repeat(32), &sha, 1, now, 501)
                .is_err()
        );
    }
    #[test]
    fn global_stop_invalidates_even_an_unexpired_lease() {
        let now = Instant::now();
        let mut c = Cache::new(1, std::sync::Arc::default());
        insert(&mut c, ID, now);
        let p = permit(&mut c, ID, now);
        c.fence.stop();
        assert!(c.read(WS, &p, now).is_err());
        assert!(c.available(now).is_err());
    }
}
