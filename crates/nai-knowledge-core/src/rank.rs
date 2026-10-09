//! Fusion ranking: exact facts lead, then lexical/vector candidates fill the
//! bounded result set, deduplicated by evidence_id and never mixing score
//! scales across providers.
use nai_contracts::v1::Evidence;
use std::collections::HashSet;

pub const MAX_RANKED: usize = 64;

/// Returns a bounded, deduplicated evidence list. Exact facts keep priority;
/// `lexical`/`vector` candidates (already time/permission-filtered) are ranked
/// after facts by their own rank. Candidates from different providers are not
/// compared by raw score; their rank position is authoritative.
pub fn fuse(
    facts: impl IntoIterator<Item = Evidence>,
    lexical: impl IntoIterator<Item = Evidence>,
    vector: impl IntoIterator<Item = Evidence>,
    top_k: usize,
) -> Vec<Evidence> {
    let top_k = top_k.min(MAX_RANKED);
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for evidence in facts.into_iter().chain(lexical).chain(vector) {
        if evidence.evidence_id.is_empty() || !seen.insert(evidence.evidence_id.clone()) {
            continue;
        }
        result.push(evidence);
        if result.len() >= top_k {
            break;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(id: &str) -> Evidence {
        Evidence {
            evidence_id: id.into(),
            document_id: "doc".into(),
            revision: "rev".into(),
            rank: 0,
            ..Default::default()
        }
    }

    #[test]
    fn facts_win_and_duplicates_are_deduped() {
        let facts = vec![evidence("fact-1"), evidence("fact-1")];
        let lexical = vec![evidence("lex-1")];
        let vector = vec![evidence("vec-1"), evidence("lex-1")];
        let fused = fuse(facts, lexical, vector, 10);
        assert_eq!(
            fused
                .iter()
                .map(|e| e.evidence_id.as_str())
                .collect::<Vec<_>>(),
            vec!["fact-1", "lex-1", "vec-1"]
        );
    }

    #[test]
    fn bounded_top_k() {
        let lexical: Vec<Evidence> = (0..100).map(|i| evidence(&format!("lex-{i}"))).collect();
        let fused = fuse(vec![], lexical, vec![], 8);
        assert_eq!(fused.len(), 8);
    }
}
