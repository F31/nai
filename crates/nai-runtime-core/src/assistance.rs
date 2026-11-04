//! Deterministic preparation FAQ lookup. Never a model, live context or delivery grant.
use crate::foundation::{Operation, Result};
use nai_contracts::workbench::WorkbenchError as Error;
use serde_json::{Value, json};
use std::collections::BTreeSet;
const STRATEGY: &str = "faq-lexical-zh-v1";
fn cjk(c: char) -> bool {
    ('\u{3400}'..='\u{9fff}').contains(&c)
}
fn terms(text: &str) -> BTreeSet<String> {
    let mut text = text.to_ascii_lowercase();
    for stop in [
        "请问",
        "这个",
        "这款",
        "杯子",
        "产品",
        "商品",
        "一下",
        "知道",
        "多大",
        "多少",
        "有多",
        "怎么样",
        "是什么",
        "是否",
        "可以",
        "能否",
        "什么",
        "怎么",
        "如何",
        "请",
        "的",
        "了",
        "吗",
        "呢",
        "啊",
        "呀",
        "吧",
        "有",
        "是",
    ] {
        text = text.replace(stop, "");
    }
    let mut result = BTreeSet::new();
    let mut previous = None;
    let mut word = String::new();
    for c in text.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_alphanumeric() {
            word.push(c);
        } else if !word.is_empty() {
            if ![
                "what", "is", "the", "a", "an", "can", "how", "much", "does", "it", "of", "please",
            ]
            .contains(&word.as_str())
            {
                result.insert(std::mem::take(&mut word));
            }
            word.clear();
        }
        if cjk(c) {
            if let Some(p) = previous {
                result.insert(format!("{p}{c}"));
            }
            previous = Some(c);
        } else {
            previous = None;
        }
    }
    result
}
fn normalized(question: &str) -> String {
    question
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .trim_end_matches(['?', '？'])
        .to_ascii_lowercase()
}
pub fn lookup(op: &Operation, product: &Value, question: &str) -> Result<Value> {
    op.check()?;
    if question.chars().count() > 256
        || question.chars().any(char::is_control)
        || normalized(question).is_empty()
    {
        return Err(Error::InvalidArgument);
    }
    if crate::preparation::validate(op, &product.to_string())? != "product_draft" {
        return Err(Error::InvalidArgument);
    }
    let matches = product["faq"]
        .as_array()
        .ok_or(Error::IntegrityFailed)?
        .iter()
        .filter(|faq| {
            faq["question"]
                .as_str()
                .is_some_and(|q| normalized(q) == normalized(question))
        })
        .collect::<Vec<_>>();
    let empty = |reason| json!({"result":reason,"faq_id":"","text":"","fact_ids":[],"evidence":[]});
    if matches.len() > 1 {
        return Ok(empty("ambiguous"));
    }
    let (faq, kind, score) = if let Some(faq) = matches.first() {
        (*faq, "exact", 1000)
    } else {
        // Conservative quotation retrieval, not semantic reasoning or instruction following.
        if ["不", "没", "无", "禁止", "忽略", "指令", "系统", "提示词"]
            .iter()
            .any(|s| question.contains(s))
        {
            return Ok(empty("no_match"));
        }
        let query = terms(question);
        if query.is_empty() {
            return Ok(empty("no_match"));
        }
        let faqs = product["faq"].as_array().ok_or(Error::IntegrityFailed)?;
        if faqs.len() > 64 {
            return Err(Error::ResourceExhausted);
        }
        let mut ranked = Vec::new();
        for faq in faqs {
            op.check()?;
            if faq["fact_ids"].as_array().is_none_or(Vec::is_empty) {
                continue;
            }
            let candidate = terms(faq["question"].as_str().ok_or(Error::IntegrityFailed)?);
            let shared = query.intersection(&candidate).count();
            if shared == 0 || shared * 2 < query.len() || shared * 2 < candidate.len() {
                continue;
            }
            let score = (2000 * shared / (query.len() + candidate.len())) as u32;
            ranked.push((faq, score));
        }
        ranked.sort_by_key(|b| std::cmp::Reverse(b.1));
        let Some(&(faq, score)) = ranked.first() else {
            return Ok(empty("no_match"));
        };
        if ranked
            .get(1)
            .is_some_and(|(_, second)| score - second < 150)
        {
            return Ok(empty("ambiguous"));
        }
        (faq, "retrieved", score)
    };
    let ids = faq["fact_ids"].as_array().ok_or(Error::IntegrityFailed)?;
    if ids.is_empty() {
        return Ok(empty("no_match"));
    }
    let facts = product["facts"].as_array().ok_or(Error::IntegrityFailed)?;
    let mut evidence = Vec::new();
    for id in ids {
        op.check()?;
        let fact = facts
            .iter()
            .find(|f| f["id"] == *id)
            .ok_or(Error::IntegrityFailed)?;
        if fact["authority"] != "owner_confirmed" {
            return Err(Error::CapabilityUnavailable);
        }
        let value = fact["value"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| fact["value"].to_string());
        evidence.push(json!({"fact_id":id,"label":fact["label"],"value":value,"unit":fact["unit"].as_str().unwrap_or(""),"currency":fact["currency"].as_str().unwrap_or(""),"source":fact["source"],"conditions":fact["conditions"],"valid_from_ms":fact["valid_from_ms"],"valid_until_ms":fact["valid_until_ms"]}));
    }
    op.check()?;
    Ok(
        json!({"result":kind,"faq_id":faq["id"],"text":faq["text"],"fact_ids":ids,"evidence":evidence,"retrieval":{"strategy":STRATEGY,"method":if kind=="exact" {"exact"} else {"lexical"},"matched_question":faq["question"],"score":score}}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn product() -> Value {
        serde_json::from_str(include_str!(
            "../../../tests/fixtures/contracts/preparation.json"
        ))
        .unwrap()
    }
    #[test]
    fn chinese_exact_and_retrieved_answers_keep_original_sources() {
        let op = Operation::new(4000).unwrap();
        let p = product();
        let r = lookup(&op, &p, " 容量 多大?").unwrap();
        assert_eq!(r["text"], p["faq"][0]["text"]);
        assert_eq!(r["evidence"][0]["source"], p["facts"][0]["source"]);
        assert_eq!(r["evidence"][0]["conditions"], "标准款");
        assert_eq!(
            lookup(&op, &p, "容量有多大").unwrap()["result"],
            "retrieved"
        );
    }
    #[test]
    fn duplicate_answers_and_unsupported_sources_do_not_guess() {
        let op = Operation::new(4000).unwrap();
        let mut p = product();
        let mut other = p["faq"][0].clone();
        other["id"] = "another".into();
        other["text"] = "600毫升".into();
        p["faq"].as_array_mut().unwrap().push(other);
        let r = lookup(&op, &p, "容量多大").unwrap();
        assert_eq!(r["result"], "ambiguous");
        assert_eq!(r["text"], "");
        p["faq"].as_array_mut().unwrap().pop();
        p["faq"][0]["fact_ids"] = json!([]);
        assert_eq!(lookup(&op, &p, "容量多大").unwrap()["result"], "no_match");
    }
    #[test]
    fn question_bounds_and_cancelled_operation_are_enforced() {
        let op = Operation::new(4000).unwrap();
        for q in [" ".into(), "？".into(), "问".repeat(257), "问题\0".into()] {
            assert_eq!(lookup(&op, &product(), &q), Err(Error::InvalidArgument));
        }
        op.cancel();
        assert_eq!(lookup(&op, &product(), "容量多大"), Err(Error::Cancelled));
    }
    #[test]
    fn fixed_chinese_question_set_preserves_quotes_or_refuses_without_guessing() {
        let cases: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/contracts/chinese-faq-evaluation.json"
        ))
        .unwrap();
        for case in cases["cases"].as_array().unwrap() {
            let mut p = product();
            for (id, label, value, q) in [
                ("material", "材质", "不锈钢", "材质是什么？"),
                ("clean", "清洗", "手洗", "怎么清洗？"),
            ] {
                let mut fact = p["facts"][0].clone();
                fact["id"] = id.into();
                fact["label"] = label.into();
                fact["value_type"] = "text".into();
                fact["value"] = value.into();
                fact.as_object_mut().unwrap().remove("unit");
                p["facts"].as_array_mut().unwrap().push(fact);
                p["faq"].as_array_mut().unwrap().push(json!({"id":format!("{id}_question"),"question":q,"text":value,"fact_ids":[id]}));
            }
            if case["duplicate_capacity"] == true {
                let mut duplicate = p["faq"][0].clone();
                duplicate["id"] = "capacity_other".into();
                duplicate["question"] = "容量是多少？".into();
                p["faq"].as_array_mut().unwrap().push(duplicate);
            }
            let q = case["question"].as_str().unwrap();
            let r = lookup(&Operation::new(4000).unwrap(), &p, q).unwrap();
            assert_eq!(r["result"], case["result"], "query: {q}");
            if case.get("faq_id").is_some() {
                assert_eq!(r["faq_id"], case["faq_id"], "query: {q}");
                let original = p["faq"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|f| f["id"] == r["faq_id"])
                    .unwrap();
                assert_eq!(r["text"], original["text"]);
                assert!(!r["evidence"].as_array().unwrap().is_empty());
                assert_eq!(r["retrieval"]["matched_question"], original["question"]);
            } else {
                assert_eq!(r["text"], "");
                assert!(r["evidence"].as_array().unwrap().is_empty());
            }
        }
    }
}
