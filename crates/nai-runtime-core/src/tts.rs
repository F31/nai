//! Short speech policy only; estimates never assert provider billing or playback.
use nai_contracts::workbench::WorkbenchError as Error;
pub const SAMPLE_RATE: u32 = 24_000;
pub const MODEL_TEST_DEADLINE_MS: u32 = 45_000;
pub const PREVIEW_DEADLINE_MS: u32 = 15_000;
pub const QWEN_LANGUAGES: &[&str] = &[
    "auto",
    "chinese",
    "english",
    "french",
    "german",
    "italian",
    "japanese",
    "korean",
    "portuguese",
    "russian",
    "spanish",
];
pub fn protocol(value: &str) -> Result<&'static str, Error> {
    match value {
        "" | "openai_speech" => Ok("openai_speech"),
        "qwen_local" => Ok("qwen_local"),
        _ => Err(Error::InvalidArgument),
    }
}
pub fn validate_profile(kind: &str, url: &str, language: &str) -> Result<bool, Error> {
    let local = protocol(kind)? == "qwen_local";
    if local {
        let url = url.strip_suffix('/').unwrap_or(url);
        let rest = url
            .strip_prefix("http://127.0.0.1:")
            .or_else(|| url.strip_prefix("http://[::1]:"))
            .ok_or(Error::InvalidArgument)?;
        let (port, path) = rest.split_once('/').ok_or(Error::InvalidArgument)?;
        if path != "v1"
            || port.is_empty()
            || !port.bytes().all(|b| b.is_ascii_digit())
            || !port.parse::<u16>().is_ok_and(|n| n > 0)
            || !QWEN_LANGUAGES.contains(&language)
        {
            return Err(Error::InvalidArgument);
        }
    } else if !language.is_empty() {
        return Err(Error::InvalidArgument);
    }
    Ok(local)
}
pub const MAX_CHARACTERS: usize = 256;
pub const MAX_AUDIO_BYTES: usize = 1_048_576;
pub const MAX_SAMPLES: usize = SAMPLE_RATE as usize * 20;
pub const MAX_SAMPLES_ANY_RATE: usize = 96_000 * 20;
pub fn characters(text: &str) -> Result<u64, Error> {
    let count = text.chars().count();
    if text.trim().is_empty()
        || text.len() > 1024
        || count > MAX_CHARACTERS
        || text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return Err(Error::InvalidArgument);
    }
    Ok(count as u64)
}
pub fn estimate(text: &str, minor_per_million: Option<u64>) -> Result<u64, Error> {
    let count = characters(text)?;
    let rate = minor_per_million.ok_or(Error::CapabilityUnavailable)?;
    if rate > 1_000_000_000 {
        return Err(Error::InvalidArgument);
    }
    u64::try_from((u128::from(count) * u128::from(rate)).div_ceil(1_000_000))
        .map_err(|_| Error::ResourceExhausted)
}
/// Unknown price reserves the explicitly approved cap; it never invents a zero estimate.
pub fn reservation(text: &str, rate: Option<u64>, cap: u64) -> Result<u64, Error> {
    characters(text)?;
    if cap == 0 || cap > 1_000_000_000 {
        return Err(Error::InvalidArgument);
    }
    let reserved = match rate {
        Some(rate) => estimate(text, Some(rate))?.max(1),
        None => cap,
    };
    if reserved > cap {
        return Err(Error::ResourceExhausted);
    }
    Ok(reserved)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_unauthenticated_endpoint_has_a_literal_loopback_boundary() {
        for url in ["http://127.0.0.1:8000/v1", "http://[::1]:8000/v1/"] {
            assert_eq!(validate_profile("qwen_local", url, "chinese"), Ok(true));
        }
        for url in [
            "http://localhost:8000/v1",
            "http://example.com:8000/v1",
            "http://127.0.0.1.evil:8000/v1",
            "http://127.0.0.1:0/v1",
            "http://127.0.0.1:65536/v1",
            "http://127.0.0.1:8000/v1?x=1",
            "http://127.0.0.1:8000/v1#x",
            "http://user@127.0.0.1:8000/v1",
            "http://127.0.0.1:8000/other",
            "http://127.0.0.1:8000/v1//",
        ] {
            assert!(
                validate_profile("qwen_local", url, "chinese").is_err(),
                "{url}"
            );
        }
        assert!(validate_profile("qwen_local", "http://127.0.0.1:8000/v1", "unknown").is_err());
        assert!(protocol("unknown").is_err());
        assert_eq!(
            validate_profile("", "https://example.invalid/v1", ""),
            Ok(false)
        );
    }
    #[test]
    fn character_estimate_is_bounded_rounded_and_unknown_is_not_free() {
        assert_eq!(characters("中文🙂"), Ok(3));
        assert_eq!(estimate("中文🙂", Some(1)), Ok(1));
        assert_eq!(estimate("中文🙂", Some(0)), Ok(0));
        assert_eq!(estimate("中文", None), Err(Error::CapabilityUnavailable));
        assert!(characters(&"中".repeat(257)).is_err());
        assert!(characters(" ").is_err());
        assert!(characters("bad\0text").is_err());
        assert!(estimate("x", Some(u64::MAX)).is_err());
    }
}

#[cfg(test)]
mod reservation_tests {
    use super::*;
    #[test]
    fn unknown_price_reserves_the_cap_while_confirmed_free_remains_distinct() {
        assert_eq!(reservation("中文", None, 5), Ok(5));
        assert_eq!(reservation("中文", Some(0), 5), Ok(1));
        assert_eq!(reservation("中文", None, 0), Err(Error::InvalidArgument));
        assert_eq!(
            reservation("中文", None, 1_000_000_001),
            Err(Error::InvalidArgument)
        );
        assert_eq!(
            reservation("中文", Some(1_000_000_000), 1),
            Err(Error::ResourceExhausted)
        );
        assert!(reservation(&"中".repeat(257), None, 5).is_err());
    }
}
