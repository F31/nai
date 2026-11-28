//! V5.6.1 Qwen3-TTS adapter: loopback-only discovery + sentence synthesis.
//!
//! The deployed Qwen3-TTS HTTP API is WAV/non-streaming. This module keeps the
//! transport honest: it reports that limitation instead of claiming streaming.

use nai_contracts::workbench::WorkbenchError as Error;
use nai_runtime_core::{foundation::Operation, narration, tts as policy};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct Config {
    pub endpoint: String,
    pub model: String,
    pub voice: String,
    pub language: String,
    pub deadline_ms: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:8000/v1".into(),
            model: "qwen3-tts-1.7b-customvoice".into(),
            voice: "vivian".into(),
            language: "chinese".into(),
            deadline_ms: policy::MODEL_TEST_DEADLINE_MS,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceInfo {
    pub endpoint: String,
    pub model: String,
    pub models: Vec<String>,
    pub voices: Vec<String>,
    pub languages: Vec<String>,
    pub health: Value,
    pub sample_rate: u32,
    pub streaming: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Probe {
    pub service: ServiceInfo,
    pub text: String,
    pub samples: usize,
    pub sample_rate: u32,
    pub audio_ms: f64,
    pub elapsed_ms: u128,
    pub real_time_factor: f64,
    pub error: Option<String>,
}

pub fn sentences(text: &str) -> Result<Vec<String>, Error> {
    narration::sentences(text)
}

/// Linear mono resampler for the two consumers: 24k TTS -> 16k lip-sync
/// features and -> 48k playback. It is intentionally small and testable; the
/// media pipeline still owns the master clock.
pub fn resample_i16(samples: &[i16], from: u32, to: u32) -> Result<Vec<i16>, Error> {
    if !(8_000..=96_000).contains(&from) || !(8_000..=96_000).contains(&to) {
        return Err(Error::InvalidArgument);
    }
    if samples.is_empty() {
        return Err(Error::InvalidArgument);
    }
    if from == to {
        return Ok(samples.to_vec());
    }
    let out_len = (samples.len() as u128 * to as u128 / from as u128) as usize;
    if out_len == 0 || out_len > policy::MAX_SAMPLES_ANY_RATE {
        return Err(Error::ResourceExhausted);
    }
    let ratio = from as f64 / to as f64;
    let mut out = Vec::with_capacity(out_len);
    for index in 0..out_len {
        let position = index as f64 * ratio;
        let left = position.floor() as usize;
        let right = (left + 1).min(samples.len() - 1);
        let fraction = position - left as f64;
        let value = samples[left] as f64 * (1.0 - fraction) + samples[right] as f64 * fraction;
        out.push(value.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16);
    }
    Ok(out)
}

#[derive(Debug, Clone)]
struct CacheEntry {
    samples: Vec<i16>,
    sample_rate: u32,
}

/// Identity of the configured synthesis source and its exact request parameters.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TtsCacheKey {
    endpoint: String,
    model: String,
    voice: String,
    language: String,
    text: String,
}

/// Small bounded cache; service/model changes require a distinct key.
pub struct TtsCache {
    capacity: usize,
    map: HashMap<TtsCacheKey, CacheEntry>,
    order: VecDeque<TtsCacheKey>,
}

impl TtsCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn key(config: &Config, text: &str) -> TtsCacheKey {
        TtsCacheKey {
            endpoint: config.endpoint.clone(),
            model: config.model.clone(),
            voice: config.voice.clone(),
            language: config.language.clone(),
            text: text.to_string(),
        }
    }

    pub fn get(&mut self, key: &TtsCacheKey) -> Option<(Vec<i16>, u32)> {
        let entry = self.map.get(key)?.clone();
        self.order.retain(|item| item != key);
        self.order.push_back(key.clone());
        Some((entry.samples, entry.sample_rate))
    }

    pub fn insert(&mut self, key: TtsCacheKey, samples: Vec<i16>, sample_rate: u32) {
        self.map.insert(
            key.clone(),
            CacheEntry {
                samples,
                sample_rate,
            },
        );
        self.order.retain(|item| item != &key);
        self.order.push_back(key);
        while self.map.len() > self.capacity {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            } else {
                break;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

pub fn synthesize_cached(
    cache: &mut TtsCache,
    config: &Config,
    text: &str,
    cancel: Operation,
) -> crate::tts::Outcome {
    let rejected = |error| crate::tts::Outcome {
        samples: Vec::new(),
        sample_rate: policy::SAMPLE_RATE,
        request_started: false,
        error: Some(error),
        http_status: None,
        failure_code: String::new(),
    };
    if let Err(error) = cancel.check() {
        return rejected(error);
    }
    let key = TtsCache::key(config, text);
    if let Some((samples, sample_rate)) = cache.get(&key) {
        if let Err(error) = cancel.check() {
            return rejected(error);
        }
        return crate::tts::Outcome {
            samples,
            sample_rate,
            request_started: false,
            error: None,
            http_status: None,
            failure_code: String::new(),
        };
    }
    let outcome = synthesize(config, text, cancel.clone());
    if outcome.error.is_none() {
        if let Err(error) = cancel.check() {
            return crate::tts::Outcome {
                request_started: outcome.request_started,
                http_status: outcome.http_status,
                ..rejected(error)
            };
        }
        cache.insert(key, outcome.samples.clone(), outcome.sample_rate);
    }
    outcome
}

pub fn synthesize(config: &Config, text: &str, cancel: Operation) -> crate::tts::Outcome {
    crate::tts::run(
        crate::tts::Request {
            protocol: "qwen_local".into(),
            language: config.language.clone(),
            base_url: config.endpoint.clone(),
            model: config.model.clone(),
            voice: config.voice.clone(),
            input: text.to_string(),
            api_key: zeroize::Zeroizing::new(Vec::new()),
            deadline_ms: config.deadline_ms,
            trace_id: String::new(),
            direct: true,
        },
        cancel,
        true,
    )
}

pub fn discover(config: &Config) -> Result<ServiceInfo, Error> {
    policy::validate_profile("qwen_local", &config.endpoint, &config.language)?;
    let endpoint = config.endpoint.trim_end_matches('/').to_string();
    let root = endpoint
        .strip_suffix("/v1")
        .unwrap_or(&endpoint)
        .to_string();
    let health = http_json(&format!("{root}/health"))?;
    let models = http_json(&format!("{endpoint}/models"))?;
    let voices = http_json(&format!("{endpoint}/voices"))?;
    let model_ids = models
        .get("data")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("id").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let voice_ids: Vec<String> = voices
        .get("voices")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let languages: Vec<String> = voices
        .get("languages")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let sample_rate = health
        .get("sample_rate")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(policy::SAMPLE_RATE);
    if !model_ids.is_empty() && !model_ids.iter().any(|id| id == &config.model) {
        return Err(Error::CapabilityUnavailable);
    }
    if !voice_ids.is_empty() && !voice_ids.iter().any(|id| id == &config.voice) {
        return Err(Error::CapabilityUnavailable);
    }
    if !languages.is_empty() && !languages.iter().any(|id| id == &config.language) {
        return Err(Error::CapabilityUnavailable);
    }
    Ok(ServiceInfo {
        endpoint,
        model: config.model.clone(),
        models: model_ids,
        voices: voice_ids,
        languages,
        health,
        sample_rate,
        streaming: false,
    })
}

pub fn probe(config: &Config, text: &str) -> Probe {
    let started = Instant::now();
    match discover(config) {
        Err(error) => Probe {
            service: ServiceInfo {
                endpoint: config.endpoint.clone(),
                model: config.model.clone(),
                models: Vec::new(),
                voices: Vec::new(),
                languages: Vec::new(),
                health: Value::Null,
                sample_rate: policy::SAMPLE_RATE,
                streaming: false,
            },
            text: text.to_string(),
            samples: 0,
            sample_rate: policy::SAMPLE_RATE,
            audio_ms: 0.0,
            elapsed_ms: started.elapsed().as_millis(),
            real_time_factor: 0.0,
            error: Some(error.as_str_name().to_string()),
        },
        Ok(service) => {
            let cancel = Operation::model_task(config.deadline_ms).ok();
            let outcome = match cancel {
                Some(cancel) => synthesize(config, text, cancel),
                None => {
                    return Probe {
                        service,
                        text: text.to_string(),
                        samples: 0,
                        sample_rate: policy::SAMPLE_RATE,
                        audio_ms: 0.0,
                        elapsed_ms: started.elapsed().as_millis(),
                        real_time_factor: 0.0,
                        error: Some("INVALID_ARGUMENT".into()),
                    };
                }
            };
            let elapsed_ms = started.elapsed().as_millis();
            let audio_ms = if outcome.sample_rate == 0 {
                0.0
            } else {
                outcome.samples.len() as f64 / outcome.sample_rate as f64 * 1000.0
            };
            Probe {
                service,
                text: text.to_string(),
                samples: outcome.samples.len(),
                sample_rate: outcome.sample_rate,
                audio_ms,
                elapsed_ms,
                real_time_factor: if audio_ms == 0.0 {
                    0.0
                } else {
                    elapsed_ms as f64 / audio_ms
                },
                error: outcome.error.map(|error| error.as_str_name().to_string()),
            }
        }
    }
}

fn http_json(url: &str) -> Result<Value, Error> {
    let parsed = reqwest::Url::parse(url).map_err(|_| Error::InvalidArgument)?;
    if parsed.scheme() != "http" || !matches!(parsed.host_str(), Some("127.0.0.1" | "[::1]")) {
        return Err(Error::InvalidArgument);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Error::CapabilityUnavailable)?;
    runtime.block_on(async {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(4))
            .build()
            .map_err(|_| Error::CapabilityUnavailable)?;
        let response = client.get(parsed).send().await.map_err(|error| {
            if error.is_timeout() {
                Error::DeadlineExceeded
            } else {
                Error::CapabilityUnavailable
            }
        })?;
        if !response.status().is_success() {
            return Err(Error::CapabilityUnavailable);
        }
        let bytes = response.bytes().await.map_err(|_| Error::OutcomeUnknown)?;
        if bytes.len() > 65536 {
            return Err(Error::ResourceExhausted);
        }
        serde_json::from_slice(&bytes).map_err(|_| Error::ProtocolError)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_target_the_v561_model() {
        let config = Config::default();
        assert_eq!(config.endpoint, "http://127.0.0.1:8000/v1");
        assert_eq!(config.model, "qwen3-tts-1.7b-customvoice");
        assert_eq!(config.voice, "vivian");
        assert_eq!(config.language, "chinese");
        assert!(
            !ServiceInfo {
                endpoint: config.endpoint.clone(),
                model: config.model.clone(),
                models: vec![],
                voices: vec![],
                languages: vec![],
                health: Value::Null,
                sample_rate: 24000,
                streaming: false,
            }
            .streaming
        );
    }

    #[test]
    fn resamples_24k_to_16k_and_48k_with_bounds() {
        let samples: Vec<i16> = (0..2400).map(|n| (n as i16).saturating_mul(3)).collect();
        let to16 = resample_i16(&samples, 24_000, 16_000).unwrap();
        let to48 = resample_i16(&samples, 24_000, 48_000).unwrap();
        assert_eq!(to16.len(), 1600);
        assert_eq!(to48.len(), 4800);
        assert!(resample_i16(&samples, 24_000, 24_000).unwrap() == samples);
        assert!(resample_i16(&[], 24_000, 16_000).is_err());
        assert!(resample_i16(&samples, 1, 24_000).is_err());
    }

    #[test]
    fn cache_is_bounded_and_keyed_by_voice_model_language_text() {
        let mut cache = TtsCache::new(2);
        let config = Config::default();
        let key = TtsCache::key(&config, "你好");
        cache.insert(key.clone(), vec![1, 2], 24_000);
        assert_eq!(cache.get(&key), Some((vec![1, 2], 24_000)));
        cache.insert(TtsCache::key(&config, "a"), vec![3], 24_000);
        cache.insert(TtsCache::key(&config, "b"), vec![4], 24_000);
        assert_eq!(cache.len(), 2);
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn cache_keys_separate_service_and_delimited_fields() {
        let base = Config::default();
        let mut other = base.clone();
        other.endpoint = "http://127.0.0.1:8001/v1".into();
        assert_ne!(
            TtsCache::key(&base, "hello"),
            TtsCache::key(&other, "hello")
        );
        let mut left = base.clone();
        left.model = "a|b".into();
        left.voice = "c".into();
        let mut right = base;
        right.model = "a".into();
        right.voice = "b|c".into();
        assert_ne!(
            TtsCache::key(&left, "hello"),
            TtsCache::key(&right, "hello")
        );
    }

    #[test]
    fn cached_audio_is_not_returned_after_cancel_or_deadline() {
        let config = Config::default();
        let mut cache = TtsCache::new(2);
        cache.insert(TtsCache::key(&config, "hello"), vec![1, 2], 24_000);
        let parent = Operation::model_task(1000).unwrap();
        let child = parent.model_child(1000).unwrap();
        parent.cancel();
        let out = synthesize_cached(&mut cache, &config, "hello", child);
        assert_eq!(out.error, Some(Error::Cancelled));
        assert!(out.samples.is_empty());
        assert!(!out.request_started);
        let expired = Operation::model_task(1).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        let out = synthesize_cached(&mut cache, &config, "hello", expired);
        assert_eq!(out.error, Some(Error::DeadlineExceeded));
        assert!(out.samples.is_empty());
        assert!(!out.request_started);
    }

    #[test]
    fn sentence_plan_reuses_runtime_policy() {
        let plan = sentences("第一句。第二句！").unwrap();
        assert_eq!(plan, vec!["第一句。".to_string(), "第二句！".to_string()]);
    }
}
