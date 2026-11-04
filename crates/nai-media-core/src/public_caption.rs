//! Exact approved text and sample-submission activation. No word timing is inferred.
use nai_contracts::{public_caption_hash, workbench::WorkbenchError as Error};
pub fn validate_source(text: &str, expected: &str) -> Result<(), Error> {
    if text.is_empty() || text.len() > 2048 || text.chars().count() > 256 || text.chars().any(|c| {
        c.is_control() && c != '\n'
            || matches!(c,'\u{200b}'..='\u{200f}'|'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
    }) {
        return Err(Error::InvalidArgument);
    }
    if public_caption_hash(text) != expected {
        return Err(Error::IntegrityFailed);
    }
    Ok(())
}
pub fn active(playing: bool, stopped: bool, submitted: u64, count: u64) -> bool {
    playing && !stopped && submitted > 0 && submitted < count
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_unicode_text_is_required_and_nothing_is_normalized() {
        let t = "容量为500毫升。";
        let h = public_caption_hash(t);
        validate_source(t, &h).unwrap();
        for t in [
            "容量为500毫升",
            "容量为500毫升。 ",
            "容量为500毫升。\u{202e}",
        ] {
            assert!(validate_source(t, &h).is_err());
        }
        assert!(validate_source(&"中".repeat(257), &"a".repeat(64)).is_err());
    }
    #[test]
    fn no_caption_before_samples_or_after_expiry_stop_or_completion() {
        assert!(!active(true, false, 0, 800));
        assert!(active(true, false, 1, 800));
        assert!(!active(false, false, 1, 800));
        assert!(!active(true, true, 1, 800));
        assert!(!active(true, false, 800, 800));
        assert!(!active(true, false, 900, 800));
    }
}
