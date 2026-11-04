//! Bounded error inspection. Never returns provider text or credentials.
use futures_util::StreamExt;
pub async fn response(response: reqwest::Response) -> String {
    let status = response.status().as_u16();
    if status == 402 {
        return "BALANCE_INSUFFICIENT".into();
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(Ok(part)) = stream.next().await {
        if bytes.len().saturating_add(part.len()) > 4096 {
            break;
        }
        bytes.extend_from_slice(&part);
    }
    classify(status, &bytes)
}
fn classify(status: u16, bytes: &[u8]) -> String {
    if status == 402 {
        return "BALANCE_INSUFFICIENT".into();
    }
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) {
        // Only a bounded error envelope is inspected, never echoed or persisted.
        for field in [
            &value["error"]["code"],
            &value["error"]["message"],
            &value["code"],
            &value["message"],
        ] {
            if let Some(s) = field.as_str() {
                let s = s.to_lowercase();
                if s == "insufficient_balance"
                    || s.contains("insufficient balance")
                    || s.contains("余额不足")
                    || s.contains("欠费")
                {
                    return "BALANCE_INSUFFICIENT".into();
                }
            }
        }
    }
    format!("HTTP_{status}")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn balance_requires_explicit_evidence_and_raw_details_are_never_returned() {
        for (status, body, want) in [
            (402, "secret-key", "BALANCE_INSUFFICIENT"),
            (
                403,
                r#"{"message":"余额不足 sensitive-key"}"#,
                "BALANCE_INSUFFICIENT",
            ),
            (
                403,
                r#"{"error":{"message":"model not allowed sensitive-key"}}"#,
                "HTTP_403",
            ),
            (401, "secret-key", "HTTP_401"),
            (429, "secret-key", "HTTP_429"),
            (500, "secret-key", "HTTP_500"),
        ] {
            assert_eq!(classify(status, body.as_bytes()), want);
        }
    }
}
