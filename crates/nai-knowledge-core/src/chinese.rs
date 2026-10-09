//! Chinese lexical tokenization for FTS candidates. CJK contiguous runs yield
//! unigrams and bigrams; ASCII runs yield word tokens. Output is deterministic
//! and bounded, never a parser of arbitrary non-text content.
use std::collections::BTreeSet;

pub const MAX_TOKENS_PER_TEXT: usize = 4096;

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF
        | 0x3040..=0x30FF | 0xAC00..=0xD7AF)
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

pub fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = BTreeSet::new();
    let mut previous_cjk = None;
    let mut word = String::new();
    for c in text.chars() {
        if is_cjk(c) {
            if !word.is_empty() {
                tokens.insert(word.to_lowercase());
                word.clear();
            }
            tokens.insert(c.to_string());
            if let Some(previous) = previous_cjk {
                tokens.insert(format!("{previous}{c}"));
            }
            previous_cjk = Some(c);
        } else if is_word(c) {
            previous_cjk = None;
            if word.len() + c.len_utf8() > 64 {
                tokens.insert(word.to_lowercase());
                word.clear();
            }
            word.push(c);
        } else {
            previous_cjk = None;
            if !word.is_empty() {
                tokens.insert(word.to_lowercase());
                word.clear();
            }
        }
        if tokens.len() >= MAX_TOKENS_PER_TEXT {
            break;
        }
    }
    if !word.is_empty() && tokens.len() < MAX_TOKENS_PER_TEXT {
        tokens.insert(word.to_lowercase());
    }
    tokens.into_iter().take(MAX_TOKENS_PER_TEXT).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_runs_yield_unigrams_and_bigrams() {
        let tokens = tokenize("价格是一千二百九十九元");
        assert!(tokens.contains(&"价格".to_string()));
        assert!(tokens.contains(&"价格".to_string()) || tokens.iter().any(|t| t.contains("格")));
        assert!(tokens.iter().any(|t| t == "价格"));
        assert!(tokens.iter().any(|t| t == "一千"));
        assert!(tokens.iter().any(|t| t == "百九"));
    }

    #[test]
    fn mixed_text_preserves_ascii_words() {
        let tokens = tokenize("本产品 SKU-2026 支持 24000Hz 采样");
        assert!(tokens.contains(&"sku-2026".to_string()));
        assert!(tokens.contains(&"24000hz".to_string()));
        assert!(tokens.iter().any(|t| t == "产品"));
    }

    #[test]
    fn output_is_bounded_and_deterministic() {
        let text = "中".repeat(200_000);
        let tokens = tokenize(&text);
        assert!(tokens.len() <= MAX_TOKENS_PER_TEXT);
        assert_eq!(tokens, tokenize(&text));
        assert!(
            tokenize(&"a".repeat(200_000))
                .iter()
                .all(|token| token.len() <= 64)
        );
    }
}
