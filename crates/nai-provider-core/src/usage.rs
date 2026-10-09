use nai_contracts::v1::BudgetCharge;
use serde_json::Value;
use std::collections::BTreeMap;

/// Maps a provider usage object (e.g. OpenAI `{prompt_tokens, total_tokens}`)
/// to an exact integer BudgetCharge. `requests` is always recorded so token-only
/// actuals never erase the attempt count (D07). Unknown/float/huge fields are
/// rejected rather than silently turned into tokens.
pub fn usage_charge(
    currency: &str,
    price_revision: &str,
    usage: &Value,
    requests: u64,
) -> Result<BudgetCharge, &'static str> {
    if currency.len() != 3
        || !currency.bytes().all(|b| b.is_ascii_uppercase())
        || price_revision.is_empty()
    {
        return Err("invalid price scope");
    }
    let object = usage.as_object().ok_or("usage is not an object")?;
    let mut units = BTreeMap::new();
    units.insert("requests".to_string(), requests);
    for (key, value) in object {
        if !matches!(
            key.as_str(),
            "tokens"
                | "prompt_tokens"
                | "completion_tokens"
                | "total_tokens"
                | "characters"
                | "audio_seconds"
                | "images"
        ) {
            return Err("unknown usage unit");
        }
        let parsed = integer(value).ok_or("non-integer usage")?;
        units.insert(key.clone(), parsed);
    }
    if units.values().any(|&value| value == 0) {
        // A zero prompt token response is not a usable billing signal.
        return Err("zero usage unit");
    }
    Ok(BudgetCharge {
        currency: currency.to_string(),
        cost_minor: 0,
        units,
        price_revision: price_revision.to_string(),
    })
}

fn integer(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .or_else(|| number.as_i64().and_then(|value| u64::try_from(value).ok())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tokens_and_requests_are_exact_and_negative_or_float_rejected() {
        let charge = usage_charge(
            "CNY",
            "price-1",
            &json!({"prompt_tokens":2,"total_tokens":5}),
            1,
        )
        .unwrap();
        assert_eq!(charge.units.get("requests"), Some(&1));
        assert_eq!(charge.units.get("total_tokens"), Some(&5));
        assert_eq!(charge.cost_minor, 0);
        assert_eq!(charge.price_revision, "price-1");
        assert!(usage_charge("CNY", "price-1", &json!({"total_tokens":-1}), 1).is_err());
        assert!(usage_charge("CNY", "price-1", &json!({"total_tokens":1.5}), 1).is_err());
        assert!(usage_charge("CNY", "price-1", &json!({"total_tokens":0}), 1).is_err());
        assert!(usage_charge("cny", "price-1", &json!({"total_tokens":1}), 1).is_err());
    }
}
