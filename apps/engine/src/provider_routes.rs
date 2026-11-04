//! App-owned capability routing. A selection is pinned for one operation;
//! failures affect future requests, never redispatch the current paid call.
use nai_contracts::v1::ErrorCode;
use nai_provider_core::{ProviderClient, ProviderProfile};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    sync::Mutex,
    time::{Duration, Instant},
};

pub struct Endpoint {
    pub profile: ProviderProfile,
    pub client: Option<ProviderClient>,
    pub voice: String,
    pub max_tokens: u32,
}
#[derive(Default)]
struct Health {
    failures: u32,
    until: Option<Instant>,
    busy: bool,
    latency: Option<Duration>,
}
pub struct Routes {
    pub endpoints: Vec<Endpoint>,
    health: Mutex<Vec<Health>>,
    threshold: u32,
    cooldown: Duration,
    latency: bool,
}
pub struct Selection<'a> {
    routes: &'a Routes,
    index: usize,
    start: Instant,
}
impl Selection<'_> {
    pub fn endpoint(&self) -> &Endpoint {
        &self.routes.endpoints[self.index]
    }
    pub fn finish(&self, result: Result<(), ErrorCode>) {
        let mut all = self.routes.health.lock().expect("route health");
        let h = &mut all[self.index];
        match result {
            Ok(()) => {
                h.failures = 0;
                h.until = None;
                let elapsed = self.start.elapsed();
                h.latency = Some(
                    h.latency
                        .map_or(elapsed, |old| old.mul_f64(0.75) + elapsed.mul_f64(0.25)),
                );
            }
            Err(
                ErrorCode::ProviderUnavailable
                | ErrorCode::DeadlineExceeded
                | ErrorCode::PermissionDenied
                | ErrorCode::Unauthenticated,
            ) => {
                h.failures = h.failures.saturating_add(1);
                if h.failures >= self.routes.threshold {
                    h.until = Some(Instant::now() + self.routes.cooldown);
                }
            }
            _ => {} // Cancellation, epoch, budget or guard failures are not vendor outages.
        }
    }
}
impl Drop for Selection<'_> {
    fn drop(&mut self) {
        self.routes.health.lock().expect("route health")[self.index].busy = false;
    }
}
impl Routes {
    pub fn build(config: &Value, capability: &str) -> Result<Self, String> {
        let route = &config["provider_routes"][capability];
        let items = route["providers"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![config["provider"].clone()]);
        let mut endpoints = Vec::new();
        for item in items {
            let profile = crate::operator::profile_from(&item)?;
            let client = crate::operator::client_from(&item, &profile).ok();
            endpoints.push(Endpoint {
                profile,
                client,
                voice: item["voice_ref"]
                    .as_str()
                    .unwrap_or(config["render"]["voice_ref"].as_str().unwrap_or(""))
                    .into(),
                max_tokens: item["max_tokens"].as_u64().unwrap_or(256) as u32,
            });
        }
        let health = Mutex::new((0..endpoints.len()).map(|_| Health::default()).collect());
        Ok(Self {
            endpoints,
            health,
            threshold: route["failure_threshold"].as_u64().unwrap_or(1) as u32,
            cooldown: Duration::from_millis(route["cooldown_ms"].as_u64().unwrap_or(30_000)),
            latency: route["strategy"].as_str() == Some("lowest_latency"),
        })
    }
    pub fn select(&self) -> Result<Selection<'_>, ErrorCode> {
        self.select_at(Instant::now())
    }
    fn select_at(&self, now: Instant) -> Result<Selection<'_>, ErrorCode> {
        let mut health = self.health.lock().map_err(|_| ErrorCode::Conflict)?;
        let eligible = |i: usize| {
            self.endpoints[i].client.is_some()
                && !health[i].busy
                && health[i].until.is_none_or(|until| now >= until)
        };
        // A cooled primary always gets one recovery probe. Unknown latency is
        // never invented; cold start uses the first configured available route.
        let probe = (0..health.len()).find(|&i| eligible(i) && health[i].until.is_some());
        let first = (0..health.len()).find(|&i| eligible(i));
        let index = probe
            .or_else(|| {
                if self.latency {
                    (0..health.len())
                        .filter(|&i| eligible(i) && health[i].latency.is_some())
                        .min_by_key(|&i| health[i].latency)
                        .or(first)
                } else {
                    first
                }
            })
            .ok_or(ErrorCode::CapabilityNotReady)?;
        health[index].busy = true;
        Ok(Selection {
            routes: self,
            index,
            start: Instant::now(),
        })
    }
}

pub fn validate(config: &Value) -> Result<(), String> {
    let Some(routes) = config.get("provider_routes") else {
        return Ok(());
    };
    let mut ids = BTreeSet::new();
    let mut envs = BTreeSet::new();
    let mut currency = None;
    for cap in ["llm", "tts"] {
        for item in routes[cap]["providers"]
            .as_array()
            .ok_or("route providers missing")?
        {
            if !ids.insert(item["id"].as_str().ok_or("route id missing")?.to_string()) {
                return Err("route IDs must be globally unique".into());
            }
            if !envs.insert(item["credential_env"].as_str().ok_or("route env missing")?) {
                return Err("route credential environment names must be unique".into());
            }
            let current = item["currency"].as_str().ok_or("route currency missing")?;
            if currency.is_some_and(|old| old != current) {
                return Err("route currencies must match; no implicit FX conversion".into());
            }
            currency = Some(current);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn routes() -> Routes {
        let endpoints = (0..3).map(|i| {
            let p = serde_json::json!({"id":format!("route{i}"),"protocol":"openai","endpoint":"http://127.0.0.1:9","model":"m","currency":"USD","price_revision":"p","timeout_ms":1000});
            let profile = crate::operator::profile_from(&p).unwrap();
            let client = Some(profile.client(nai_security::Secret::new(b"fixture".to_vec()).unwrap()).unwrap());
            Endpoint { profile, client, voice: "v".into(), max_tokens: 16 }
        }).collect();
        Routes {
            endpoints,
            health: Mutex::new((0..3).map(|_| Health::default()).collect()),
            threshold: 1,
            cooldown: Duration::from_secs(30),
            latency: false,
        }
    }
    #[test]
    fn failure_changes_next_request_and_single_probe_recovers_primary() {
        let r = routes();
        let p = r.select().unwrap();
        assert_eq!(p.index, 0);
        p.finish(Err(ErrorCode::ProviderUnavailable));
        drop(p);
        assert_eq!(r.select().unwrap().index, 1);
        let future = Instant::now() + Duration::from_secs(31);
        let probe = r.select_at(future).unwrap();
        assert_eq!(probe.index, 0);
        assert_ne!(r.select_at(future).unwrap().index, 0);
        probe.finish(Ok(()));
        drop(probe);
        assert_eq!(r.select().unwrap().index, 0);
    }
    #[test]
    fn cancellation_missing_credentials_and_all_open_are_explicit() {
        let mut r = routes();
        r.endpoints[0].client = None;
        let p = r.select().unwrap();
        assert_eq!(p.index, 1);
        p.finish(Err(ErrorCode::Cancelled));
        drop(p);
        assert_eq!(r.select().unwrap().index, 1);
        for _ in 0..2 {
            let p = r.select().unwrap();
            p.finish(Err(ErrorCode::ProviderUnavailable));
        }
        assert!(matches!(r.select(), Err(ErrorCode::CapabilityNotReady)));
    }
    #[test]
    fn latency_selection_uses_only_observed_values_and_threshold_is_bounded() {
        let mut r = routes();
        r.latency = true;
        r.threshold = 2;
        {
            let mut h = r.health.lock().unwrap();
            h[0].latency = Some(Duration::from_millis(200));
            h[1].latency = Some(Duration::from_millis(50));
        }
        let p = r.select().unwrap();
        assert_eq!(p.index, 1);
        p.finish(Err(ErrorCode::ProviderUnavailable));
        drop(p);
        assert_eq!(r.select().unwrap().index, 1);
    }
}
