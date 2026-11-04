use nai_contracts::workbench::{ReviewFaq, WorkbenchError as Error};
use serde_json::{Value, json};
use std::collections::BTreeSet;
pub type Result<T> = std::result::Result<T, Error>;
pub struct Change {
    pub product: Value,
    pub before: ReviewFaq,
}
fn identifier(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 48
        && b[0].is_ascii_lowercase()
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_')
}
fn text(s: &str, max: usize) -> bool {
    !s.trim().is_empty()
        && s.chars().count() <= max
        && !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
}
pub fn validate_proposal(p: &ReviewFaq) -> Result<()> {
    if !identifier(&p.id)
        || !text(&p.question, 256)
        || !text(&p.text, 1024)
        || p.fact_ids.is_empty()
        || p.fact_ids.len() > 16
        || p.fact_ids.iter().any(|x| !identifier(x))
        || p.fact_ids.iter().collect::<BTreeSet<_>>().len() != p.fact_ids.len()
    {
        return Err(Error::InvalidArgument);
    }
    Ok(())
}
pub fn faq(v: &Value) -> Result<ReviewFaq> {
    Ok(ReviewFaq {
        id: v["id"].as_str().ok_or(Error::IntegrityFailed)?.into(),
        question: v["question"].as_str().ok_or(Error::IntegrityFailed)?.into(),
        text: v["text"].as_str().ok_or(Error::IntegrityFailed)?.into(),
        fact_ids: v["fact_ids"]
            .as_array()
            .ok_or(Error::IntegrityFailed)?
            .iter()
            .map(|x| x.as_str().map(str::to_owned).ok_or(Error::IntegrityFailed))
            .collect::<Result<_>>()?,
    })
}
pub fn proposal_value(p: &ReviewFaq) -> Value {
    json!({"id":p.id,"question":p.question,"text":p.text,"fact_ids":p.fact_ids})
}
pub fn before(product: &Value, id: &str) -> Result<ReviewFaq> {
    let entries = product["faq"].as_array().ok_or(Error::IntegrityFailed)?;
    entries
        .iter()
        .find(|v| v["id"] == id)
        .map(faq)
        .unwrap_or_else(|| {
            Ok(ReviewFaq {
                id: id.into(),
                ..Default::default()
            })
        })
}
fn name(v: &Value) -> Result<Value> {
    let s = v["name"].as_str().ok_or(Error::IntegrityFailed)?;
    Ok(format!("{}（下场草稿）", s.chars().take(120).collect::<String>()).into())
}
pub fn plan(product: &Value, show: &Value, p: &ReviewFaq) -> Result<Change> {
    validate_proposal(p)?;
    if product["kind"] != "product_draft" || show["kind"] != "show_draft" {
        return Err(Error::InvalidArgument);
    }
    let facts = product["facts"]
        .as_array()
        .ok_or(Error::IntegrityFailed)?
        .iter()
        .filter_map(|f| f["id"].as_str())
        .collect::<BTreeSet<_>>();
    if p.fact_ids.iter().any(|id| !facts.contains(id.as_str())) {
        return Err(Error::RevisionConflict);
    }
    let old = before(product, &p.id)?;
    let normalize = |s: &str| s.split_whitespace().collect::<String>().to_lowercase();
    let mut product = product.clone();
    let new_name = name(&product)?;
    let entries = product["faq"]
        .as_array_mut()
        .ok_or(Error::IntegrityFailed)?;
    if entries.iter().any(|e| {
        e["id"] != p.id
            && e["question"]
                .as_str()
                .is_some_and(|q| normalize(q) == normalize(&p.question))
    }) {
        return Err(Error::RevisionConflict);
    }
    if let Some(entry) = entries.iter_mut().find(|v| v["id"] == p.id) {
        *entry = proposal_value(p);
    } else {
        if entries.len() >= 32 {
            return Err(Error::ResourceExhausted);
        }
        entries.push(proposal_value(p));
    }
    product["name"] = new_name;
    // A clone must retain every authoritative field, condition and unrelated expression.
    Ok(Change {
        product,
        before: old,
    })
}
pub fn show_draft(show: &Value, product_ref: Value) -> Result<Value> {
    let mut v = show.clone();
    v["name"] = name(show)?;
    v["product"] = product_ref;
    Ok(v)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> (Value, Value, ReviewFaq) {
        (
            json!({"kind":"product_draft","name":"水杯","facts":[{"id":"capacity","value":"500","conditions":["标准款"]}],"faq":[{"id":"capacity","question":"多大？","text":"容量500毫升。","fact_ids":["capacity"]}],"talk_tracks":[{"id":"opening","text":"容量500毫升。"}],"prohibitions":["不得夸大"]}),
            json!({"kind":"show_draft","name":"本场","product":{"object_id":"old"},"host":{"object_id":"host"},"cues":[{"id":"intro"}]}),
            ReviewFaq {
                id: "capacity".into(),
                question: "容量多少？".into(),
                text: "标准款容量500毫升。".into(),
                fact_ids: vec!["capacity".into()],
            },
        )
    }
    #[test]
    fn faq_change_preserves_facts_conditions_unrelated_tracks_and_source() {
        let (p, s, n) = input();
        let old = p.clone();
        let c = plan(&p, &s, &n).unwrap();
        assert_eq!(p, old);
        assert_eq!(c.product["facts"], p["facts"]);
        assert_eq!(c.product["talk_tracks"], p["talk_tracks"]);
        assert_eq!(c.product["prohibitions"], p["prohibitions"]);
        assert_eq!(c.before.question, "多大？");
        let next = show_draft(&s, json!({"object_id":"new","revision":1,"sha256":"hash"})).unwrap();
        assert_eq!(next["host"], s["host"]);
        assert_eq!(next["cues"], s["cues"]);
        assert_eq!(s["product"]["object_id"], "old");
    }
    #[test]
    fn missing_fact_duplicate_question_and_limits_are_not_silent_merges() {
        let (p, s, mut n) = input();
        n.fact_ids = vec!["unknown".into()];
        assert_eq!(plan(&p, &s, &n).err(), Some(Error::RevisionConflict));
        n.fact_ids = vec!["capacity".into()];
        n.id = "new".into();
        n.question = " 多大？ ".into();
        assert_eq!(plan(&p, &s, &n).err(), Some(Error::RevisionConflict));
        n.text = " ".into();
        assert!(validate_proposal(&n).is_err());
    }
    #[test]
    fn applying_same_faq_identity_does_not_duplicate_entries() {
        let (p, s, n) = input();
        let first = plan(&p, &s, &n).unwrap().product;
        let next = plan(&first, &s, &n).unwrap().product;
        assert_eq!(next["faq"].as_array().unwrap().len(), 1);
    }
}
