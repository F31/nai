//! Golden-set payload gate: where the hundred questions live, and whether they
//! have arrived.
//!
//! §30.1 gives the RAG golden cases their own directory, and §6 asks for at
//! least a hundred questions close to the real livestream — but §6 marks that
//! whole threshold a *design value*, and §5.9 makes the set a field of the
//! scenario package. Nothing in this repository can therefore invent the
//! questions: they are supplied, per scenario, by whoever owns the scenario.
//! What this module can do is make "supplied" mean something concrete.
//!
//! Three rules shape it.
//!
//! First, **one declared location and one declared format**. A case set is a
//! `*.json` file under [`PAYLOAD_DIR`] that names its schema revision and its
//! scenario version. A file that names a revision this build does not know is
//! refused rather than guessed at.
//!
//! Second, **"never supplied" is not "passed"**, and it is not "failed"
//! either. An empty payload directory is reported as `CAPABILITY_NOT_READY`
//! naming the directory that was searched — the same shape the kernel uses for
//! an unmeasured capacity. A payload that is present but broken is reported as
//! defects. The two are kept apart so neither can stand in for the other.
//!
//! Third, **the payload boundary is where a promoted case must name its
//! confirmer**. §5.9 allows a bad case into the golden set only through a human
//! confirmation, so a `promoted` origin with no confirmer is refused while
//! reading the file — before any run exists in which the shortfall could be
//! blamed on the system instead of on the set.

use nai_kernel::golden::{
    CaseOrigin, Category, DocRef, Expectation, ExpectedAnswer, Finding, GoldenCase, GoldenSet,
};
use serde_json::{Map, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// §30.1 names this directory for the RAG golden cases. It is the one place a
/// case set may live, so "where are the hundred questions" has a single answer.
pub const PAYLOAD_DIR: &str = "tests/rag-golden";

/// The schema revision every case set must declare. A file that does not name
/// this revision is refused rather than interpreted leniently.
const CASE_SET_SCHEMA: u64 = 2;

const SET_KEYS: [&str; 3] = ["case_set_schema", "scenario_version", "cases"];
const CASE_KEYS: [&str; 6] = [
    "id",
    "category",
    "question",
    "expectation",
    "expected",
    "origin",
];

/// The key that discriminates an expectation's or an origin's variant. It is
/// the only key every shape shares.
const KIND: &str = "kind";

/// A structural problem with the payload *file*.
///
/// It is deliberately not the same type as a [`Finding`]: a defect means the
/// file cannot be read as a case set at all, while a finding means the set
/// loaded and its own rules were audited. A caller can tell "the payload is
/// malformed" from "the payload is well-formed but not yet qualifying".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Defect {
    pub file: String,
    pub token: &'static str,
    pub detail: String,
}

/// One file's audit, after it loaded successfully.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetAudit {
    pub file: String,
    pub scenario_version: String,
    pub cases: usize,
    /// Structural findings from the set itself (too few cases, an uncovered
    /// category, an expectation that cannot belong to its category). Empty is
    /// the qualifying state.
    pub findings: Vec<Finding>,
}

/// The whole payload directory: what was found, and what is wrong with it.
#[derive(Debug, Clone, Default)]
pub struct PayloadAudit {
    /// How many `*.json` files the directory held. Zero is the honest answer to
    /// "has the payload ever been supplied", and it is reported as such.
    pub files: usize,
    pub sets: Vec<SetAudit>,
    pub defects: Vec<Defect>,
}

impl PayloadAudit {
    /// True when nothing has been supplied at all.
    pub fn is_unsupplied(&self) -> bool {
        self.files == 0
    }

    /// True when every supplied file loaded and every loaded set qualified.
    pub fn passed(&self) -> bool {
        !self.is_unsupplied()
            && self.defects.is_empty()
            && self.sets.iter().all(|set| set.findings.is_empty())
    }

    /// Structural findings across every loaded set.
    pub fn findings(&self) -> usize {
        self.sets.iter().map(|set| set.findings.len()).sum()
    }
}

/// Collects defects while keeping the file name attached to each one, so no
/// check has to thread both the path and the message through by hand.
struct Report<'a> {
    file: &'a str,
    defects: Vec<Defect>,
}

impl<'a> Report<'a> {
    fn new(file: &'a str) -> Self {
        Self {
            file,
            defects: Vec::new(),
        }
    }

    fn defect(&mut self, token: &'static str, detail: impl Into<String>) {
        self.defects.push(Defect {
            file: self.file.to_owned(),
            token,
            detail: detail.into(),
        });
    }

    /// Reject unknown keys rather than ignoring them: a misspelled field would
    /// otherwise read as an absent one and silently change what the case says.
    fn keys(&mut self, object: &Map<String, Value>, allowed: &[&str], at: &str) {
        for key in object.keys() {
            if !allowed.contains(&key.as_str()) {
                self.defect("unknown_field", format!("{at}.{key}"));
            }
        }
    }
}

fn as_object<'v>(
    report: &mut Report<'_>,
    value: &'v Value,
    at: &str,
) -> Option<&'v Map<String, Value>> {
    match value {
        Value::Object(map) => Some(map),
        _ => {
            report.defect("field_type", format!("{at} must be an object"));
            None
        }
    }
}

/// A required non-empty string. Blank counts as absent: a confirmer of `""` or
/// `"  "` must not read as a confirmation.
fn text(
    report: &mut Report<'_>,
    object: &Map<String, Value>,
    key: &str,
    at: &str,
) -> Option<String> {
    match object.get(key) {
        None => {
            report.defect("field_missing", at);
            None
        }
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.clone()),
        Some(Value::String(_)) => {
            report.defect("field_empty", format!("{at} must not be blank"));
            None
        }
        Some(_) => {
            report.defect("field_type", format!("{at} must be a non-empty string"));
            None
        }
    }
}

fn list<'v>(
    report: &mut Report<'_>,
    object: &'v Map<String, Value>,
    key: &str,
    at: &str,
) -> Option<Vec<&'v Value>> {
    match object.get(key) {
        None => {
            report.defect("field_missing", at);
            None
        }
        Some(Value::Array(items)) if !items.is_empty() => Some(items.iter().collect()),
        Some(Value::Array(_)) => {
            report.defect("field_empty", format!("{at} must not be empty"));
            None
        }
        Some(_) => {
            report.defect("field_type", format!("{at} must be an array"));
            None
        }
    }
}

fn map<'v>(
    report: &mut Report<'_>,
    object: &'v Map<String, Value>,
    key: &str,
    at: &str,
) -> Option<&'v Map<String, Value>> {
    match object.get(key) {
        None => {
            report.defect("field_missing", at);
            None
        }
        Some(Value::Object(value)) => Some(value),
        Some(_) => {
            report.defect("field_type", format!("{at} must be an object"));
            None
        }
    }
}

/// A list of document references. An empty list is refused here rather than
/// left to the run: a case that requires no evidence cannot be scored, so it is
/// a broken case, not a failing system.
fn refs(
    report: &mut Report<'_>,
    object: &Map<String, Value>,
    key: &str,
    at: &str,
) -> Option<Vec<DocRef>> {
    let items = list(report, object, key, at)?;
    let mut refs = Vec::new();
    for (index, item) in items.iter().enumerate() {
        match item {
            Value::String(id) if !id.trim().is_empty() => refs.push(DocRef::new(id)),
            Value::String(_) => {
                report.defect("field_empty", format!("{at}[{index}] must not be blank"))
            }
            _ => report.defect("field_type", format!("{at}[{index}] must be a string")),
        }
    }
    Some(refs)
}

fn strings(
    report: &mut Report<'_>,
    object: &Map<String, Value>,
    key: &str,
    at: &str,
) -> Option<Vec<String>> {
    let items = list(report, object, key, at)?;
    let mut values = Vec::new();
    for (index, item) in items.iter().enumerate() {
        match item {
            Value::String(value) if !value.trim().is_empty() => values.push(value.clone()),
            Value::String(_) => {
                report.defect("field_empty", format!("{at}[{index}] must not be blank"))
            }
            _ => report.defect("field_type", format!("{at}[{index}] must be a string")),
        }
    }
    Some(values)
}

fn parse_category(
    report: &mut Report<'_>,
    object: &Map<String, Value>,
    at: &str,
) -> Option<Category> {
    let label = text(report, object, "category", at)?;
    match Category::ALL
        .iter()
        .find(|category| category.label() == label)
    {
        Some(category) => Some(*category),
        None => {
            report.defect("category_unknown", format!("{at} = {label}"));
            None
        }
    }
}

/// What a case asserts, in the payload's own vocabulary. Each variant checks
/// exactly the keys it needs, so an `evidence` shape carrying
/// `forbidden_claims` is an unknown field rather than something silently kept.
fn parse_expectation(
    report: &mut Report<'_>,
    object: &Map<String, Value>,
    key: &str,
    at: &str,
) -> Option<Expectation> {
    let object = map(report, object, key, at)?;
    let kind_at = format!("{at}.kind");
    let kind = text(report, object, KIND, &kind_at)?;
    match kind.as_str() {
        "evidence" => {
            report.keys(object, &["kind", "required"], at);
            let required_at = format!("{at}.required");
            let required = refs(report, object, "required", &required_at)?;
            Some(Expectation::Evidence { required })
        }
        "high_risk_fact" => {
            report.keys(object, &["kind", "forbidden_claims"], at);
            let claims_at = format!("{at}.forbidden_claims");
            let forbidden_claims = strings(report, object, "forbidden_claims", &claims_at)?;
            Some(Expectation::HighRiskFact { forbidden_claims })
        }
        "must_clarify_or_escalate" => {
            report.keys(object, &["kind"], at);
            Some(Expectation::MustClarifyOrEscalate)
        }
        "must_not_act" => {
            report.keys(object, &["kind"], at);
            Some(Expectation::MustNotAct)
        }
        "must_cancel" => {
            report.keys(object, &["kind"], at);
            Some(Expectation::MustCancel)
        }
        other => {
            report.defect("expectation_kind_unknown", format!("{kind_at} = {other}"));
            None
        }
    }
}

/// The expected answer of one case. `must_include` is required and non-empty:
/// an expected answer with nothing to check would let every answer pass or let
/// none be judged, and both quietly turn the generation-side check off.
/// `must_exclude` is optional, but when present it too carries at least one
/// phrase — an empty list is a defect here as everywhere else.
fn parse_expected(
    report: &mut Report<'_>,
    object: &Map<String, Value>,
    key: &str,
    at: &str,
) -> Option<ExpectedAnswer> {
    let object = map(report, object, key, at)?;
    let kind_at = format!("{at}.kind");
    let kind = text(report, object, KIND, &kind_at)?;
    match kind.as_str() {
        "reference" => {
            report.keys(object, &["kind", "must_include", "must_exclude"], at);
            let include_at = format!("{at}.must_include");
            let must_include = strings(report, object, "must_include", &include_at)?;
            let must_exclude = if object.contains_key("must_exclude") {
                strings(
                    report,
                    object,
                    "must_exclude",
                    &format!("{at}.must_exclude"),
                )?
            } else {
                Vec::new()
            };
            Some(ExpectedAnswer {
                must_include,
                must_exclude,
            })
        }
        other => {
            report.defect("expected_kind_unknown", format!("{kind_at} = {other}"));
            None
        }
    }
}

/// Where a case came from. `promoted` demands a confirmer here, at the payload
/// boundary, because §5.9 makes the human confirmation the whole difference
/// between a bad case and a case.
fn parse_origin(
    report: &mut Report<'_>,
    object: &Map<String, Value>,
    at: &str,
) -> Option<CaseOrigin> {
    let object = map(report, object, "origin", at)?;
    let kind_at = format!("{at}.kind");
    let kind = text(report, object, KIND, &kind_at)?;
    match kind.as_str() {
        "authored" => {
            report.keys(object, &["kind"], at);
            Some(CaseOrigin::Authored)
        }
        "promoted" => {
            report.keys(object, &["kind", "confirmer"], at);
            let confirmer = text(report, object, "confirmer", &format!("{at}.confirmer"))?;
            Some(CaseOrigin::Promoted { confirmer })
        }
        other => {
            report.defect("origin_kind_unknown", format!("{kind_at} = {other}"));
            None
        }
    }
}

fn parse_case(report: &mut Report<'_>, value: &Value, index: usize) -> Option<GoldenCase> {
    let at = format!("cases[{index}]");
    let object = as_object(report, value, &at)?;
    report.keys(object, &CASE_KEYS, &at);

    let id = text(report, object, "id", &format!("{at}.id"))?;
    let category = parse_category(report, object, &format!("{at}.category"))?;
    let question = text(report, object, "question", &format!("{at}.question"))?;
    let expectation =
        parse_expectation(report, object, "expectation", &format!("{at}.expectation"))?;
    // Required, not optional: without it the generation side can never be
    // judged, so a case that omits it is structurally broken.
    let expected = parse_expected(report, object, "expected", &format!("{at}.expected"))?;
    let origin = parse_origin(report, object, &format!("{at}.origin"))?;

    Some(GoldenCase {
        id,
        category,
        question,
        expectation,
        expected: Some(expected),
        origin,
    })
}

/// Read one case set and audit it. Every structural problem is collected, so a
/// broken payload is reported in full rather than one error at a time.
pub fn audit_case_set(file: &str, source: &str) -> Result<SetAudit, Vec<Defect>> {
    let (set, findings) = parse_case_set(file, source)?;
    Ok(SetAudit {
        file: file.to_owned(),
        scenario_version: set.scenario_version.clone(),
        cases: set.len(),
        findings,
    })
}

/// Read one case set into the kernel's set type, for a caller that is going to
/// **run** it rather than only audit it. It applies the same reading rules as
/// [`audit_case_set`], because both call [`parse_case_set`] — a file that loads
/// here is exactly a file the payload gate accepts, so the audit and the runner
/// can never disagree about what a valid set is.
pub fn read_case_set(file: &str, source: &str) -> Result<GoldenSet, Vec<Defect>> {
    parse_case_set(file, source).map(|(set, _)| set)
}

/// The one reader the audit and the runner share. Every structural rule lives
/// here, so there is a single answer to "what is a valid case set".
fn parse_case_set(file: &str, source: &str) -> Result<(GoldenSet, Vec<Finding>), Vec<Defect>> {
    let mut report = Report::new(file);
    let root = match serde_json::from_str::<Value>(source) {
        Ok(value) => value,
        Err(error) => {
            report.defect("json_invalid", error.to_string());
            return Err(report.defects);
        }
    };
    let Some(object) = as_object(&mut report, &root, "$") else {
        return Err(report.defects);
    };
    report.keys(object, &SET_KEYS, "$");

    match object.get("case_set_schema").and_then(Value::as_u64) {
        Some(found) if found == CASE_SET_SCHEMA => {}
        Some(found) => report.defect(
            "case_set_schema_unsupported",
            format!("$.case_set_schema = {found}, expected {CASE_SET_SCHEMA}"),
        ),
        None => report.defect(
            "case_set_schema_unsupported",
            format!("$.case_set_schema must be the integer {CASE_SET_SCHEMA}"),
        ),
    }

    let scenario_version = text(
        &mut report,
        object,
        "scenario_version",
        "$.scenario_version",
    );
    let cases = list(&mut report, object, "cases", "$.cases");
    let (Some(scenario_version), Some(cases)) = (scenario_version, cases) else {
        return Err(report.defects);
    };

    let mut parsed = Vec::new();
    for (index, value) in cases.iter().enumerate() {
        if let Some(case) = parse_case(&mut report, value, index) {
            parsed.push(case);
        }
    }
    if !report.defects.is_empty() {
        return Err(report.defects);
    }

    let mut set = GoldenSet::new(scenario_version);
    for case in parsed {
        let id = case.id.clone();
        // The only way `add` fails is a duplicate id, and a duplicated case
        // would double-weight one question in the recall ratio.
        if set.add(case).is_err() {
            report.defect("duplicate_case_id", id);
        }
    }
    if !report.defects.is_empty() {
        return Err(report.defects);
    }

    let findings = set.audit_coverage();
    Ok((set, findings))
}

/// Every `*.json` directly under `dir`, in name order. A missing directory is
/// not an error here: it produces an empty audit, which [`check_dir`] turns
/// into an explicit "not supplied" refusal.
pub fn audit_directory(dir: &Path) -> Result<PayloadAudit, String> {
    let mut audit = PayloadAudit::default();
    for (name, text) in case_set_files(dir)? {
        audit.files += 1;
        match audit_case_set(&name, &text) {
            Ok(set) => audit.sets.push(set),
            Err(defects) => audit.defects.extend(defects),
        }
    }
    Ok(audit)
}

/// Every `*.json` directly under `dir`, in name order, as `(file name,
/// contents)`. A directory that does not exist yields an empty list — "not
/// supplied" is a conclusion the caller reaches, not an error raised here.
///
/// It exists so a caller that is going to **run** the cases reads them through
/// the same scan the payload gate uses, rather than growing a second one.
pub fn case_set_files(dir: &Path) -> Result<Vec<(String, String)>, String> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let entries =
        fs::read_dir(dir).map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut files = Vec::new();
    for path in paths {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unnamed>")
            .to_owned();
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        files.push((name, text));
    }
    Ok(files)
}

/// `cargo xtask check --golden [--dir <path>]`.
pub fn check(root: &Path, dir: Option<&str>) -> Result<(), String> {
    let relative = dir.unwrap_or(PAYLOAD_DIR);
    check_dir(&root.join(relative), relative)
}

/// Audit the payload directory at `path` and refuse unless it is both supplied
/// and sound. `label` is what the refusal names, so a `--dir` override is
/// reported as the caller asked for it.
pub fn check_dir(path: &Path, label: &str) -> Result<(), String> {
    let audit = audit_directory(path)?;
    let cases: usize = audit.sets.iter().map(|set| set.cases).sum();
    println!("golden case set payload: {label}");
    println!("  files: {}", audit.files);
    for set in &audit.sets {
        println!(
            "  {} [{}] {} case(s)",
            set.file, set.scenario_version, set.cases
        );
        for finding in &set.findings {
            println!("    {}: {}", finding.token, finding.subject);
        }
    }
    for defect in &audit.defects {
        println!("  {} {}: {}", defect.file, defect.token, defect.detail);
    }

    if audit.is_unsupplied() {
        return Err(format!(
            "CAPABILITY_NOT_READY: no golden case set under {label}; the §6 questions are a \
             scenario-package payload, supplied per scenario and never fabricated here \
             (see {PAYLOAD_DIR}/README.md)"
        ));
    }
    if !audit.defects.is_empty() {
        return Err(format!(
            "{label}: {} structural defect(s) in the golden case set payload",
            audit.defects.len()
        ));
    }
    let findings = audit.findings();
    if findings > 0 {
        return Err(format!(
            "{label}: the golden case set does not meet the structural rules ({findings} finding(s))"
        ));
    }
    println!(
        "golden case set payload passed: {} file(s), {cases} case(s), eight categories declared",
        audit.sets.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATEGORIES: [&str; 8] = [
        "normal_answer",
        "fact_update",
        "injection",
        "conflict",
        "privilege_escalation",
        "no_answer",
        "cancelled",
        "tool_return_attack",
    ];

    /// An expectation this category accepts, so a generated set has no
    /// deliberate case-configuration defect unless a test wants one.
    fn expectation_for(category: &str, name: &str) -> String {
        match category {
            "normal_answer" => format!(r#"{{"kind":"evidence","required":["{name}-doc"]}}"#),
            "fact_update" => format!(r#"{{"kind":"evidence","required":["{name}-fact"]}}"#),
            "conflict" => format!(r#"{{"kind":"evidence","required":["{name}-a","{name}-b"]}}"#),
            "injection" | "privilege_escalation" | "tool_return_attack" => {
                r#"{"kind":"must_not_act"}"#.to_owned()
            }
            "no_answer" => r#"{"kind":"must_clarify_or_escalate"}"#.to_owned(),
            "cancelled" => r#"{"kind":"must_cancel"}"#.to_owned(),
            other => panic!("no expectation is defined for {other}"),
        }
    }

    /// A set of `count` cases cycling through all eight categories, so `count`
    /// at 8 covers everything and `count` at 100 also meets the §6 minimum.
    fn set_json(scenario_version: &str, count: usize) -> String {
        let mut cases = Vec::new();
        for index in 0..count {
            let category = CATEGORIES[index % CATEGORIES.len()];
            let id = format!("{category}-{index}");
            cases.push(format!(
                r#"{{"id":"{id}","category":"{category}","question":"q {index}","expectation":{},"expected":{{"kind":"reference","must_include":["ok"]}},"origin":{{"kind":"authored"}}}}"#,
                expectation_for(category, &id)
            ));
        }
        format!(
            r#"{{"case_set_schema":2,"scenario_version":"{scenario_version}","cases":[{}]}}"#,
            cases.join(",")
        )
    }

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("nai-xtask-golden-{name}"));
        let _ = fs::remove_dir_all(&path);
        path
    }

    fn tokens(defects: &[Defect]) -> Vec<&'static str> {
        defects.iter().map(|defect| defect.token).collect()
    }

    #[test]
    fn a_short_set_is_named_as_short() {
        let audit = audit_case_set("set.json", &set_json("scenario-1", 8)).unwrap();
        assert_eq!(audit.cases, 8);
        assert_eq!(audit.scenario_version, "scenario-1");
        assert_eq!(audit.findings.len(), 1);
        assert_eq!(audit.findings[0].token, "below_minimum_cases");
        assert_eq!(audit.findings[0].subject, "8 of 100");
    }

    #[test]
    fn a_full_covered_set_qualifies() {
        let audit = audit_case_set("set.json", &set_json("scenario-1", 100)).unwrap();
        assert_eq!(audit.cases, 100);
        assert!(audit.findings.is_empty());
    }

    #[test]
    fn a_hundred_cases_of_one_category_leave_seven_uncovered() {
        let mut cases = Vec::new();
        for index in 0..100 {
            let id = format!("normal-{index}");
            cases.push(format!(
                r#"{{"id":"{id}","category":"normal_answer","question":"q","expectation":{{"kind":"evidence","required":["d"]}},"expected":{{"kind":"reference","must_include":["ok"]}},"origin":{{"kind":"authored"}}}}"#
            ));
        }
        let text = format!(
            r#"{{"case_set_schema":2,"scenario_version":"s","cases":[{}]}}"#,
            cases.join(",")
        );
        let audit = audit_case_set("set.json", &text).unwrap();
        // Enough questions, so no shortfall; seven categories have no case.
        assert!(
            audit
                .findings
                .iter()
                .all(|finding| finding.token == "category_uncovered")
        );
        assert_eq!(audit.findings.len(), 7);
    }

    #[test]
    fn an_incompatible_expectation_points_at_the_case() {
        let text = r#"{"case_set_schema":2,"scenario_version":"s","cases":[{"id":"n-1","category":"no_answer","question":"q","expectation":{"kind":"evidence","required":["d"]},"expected":{"kind":"reference","must_include":["ok"]},"origin":{"kind":"authored"}}]}"#;
        let audit = audit_case_set("set.json", text).unwrap();
        assert!(
            audit
                .findings
                .iter()
                .any(|finding| finding.token == "expectation_category_mismatch"
                    && finding.subject == "n-1")
        );
    }

    #[test]
    fn unreadable_json_is_one_defect() {
        let defects = audit_case_set("set.json", "{").unwrap_err();
        assert_eq!(tokens(&defects), vec!["json_invalid"]);
        assert_eq!(defects[0].file, "set.json");
    }

    #[test]
    fn unknown_keys_and_empty_lists_are_named_with_their_path() {
        let text = r#"{"case_set_schema":2,"scenario_version":"s","cases":[],"extra":1}"#;
        let defects = audit_case_set("set.json", text).unwrap_err();
        assert!(
            defects
                .iter()
                .any(|defect| defect.token == "unknown_field" && defect.detail == "$.extra")
        );
        assert!(
            defects.iter().any(|defect| defect.token == "field_empty"
                && defect.detail == "$.cases must not be empty")
        );
    }

    #[test]
    fn an_unsupported_schema_revision_is_refused_not_guessed() {
        // Revision 2 is the one this build knows.
        let current = r#"{"case_set_schema":2,"scenario_version":"s","cases":[{"id":"a","category":"no_answer","question":"q","expectation":{"kind":"must_clarify_or_escalate"},"expected":{"kind":"reference","must_include":["ok"]},"origin":{"kind":"authored"}}]}"#;
        assert!(audit_case_set("set.json", current).is_ok());

        // The previous revision is refused rather than read leniently. Reading
        // it would turn a case set that never declares an expected answer into
        // one that has "no requirement", which is not the same thing.
        for (revision, value) in [(1u64, "1"), (3, "3")] {
            let text = format!(
                r#"{{"case_set_schema":{value},"scenario_version":"s","cases":[{{"id":"a","category":"no_answer","question":"q","expectation":{{"kind":"must_clarify_or_escalate"}},"expected":{{"kind":"reference","must_include":["ok"]}},"origin":{{"kind":"authored"}}}}]}}"#
            );
            let defects = audit_case_set("set.json", &text).unwrap_err();
            assert_eq!(tokens(&defects), vec!["case_set_schema_unsupported"]);
            assert_eq!(
                defects[0].detail,
                format!("$.case_set_schema = {revision}, expected 2")
            );
        }
    }

    #[test]
    fn a_case_without_an_expected_answer_is_a_defect() {
        let text = r#"{"case_set_schema":2,"scenario_version":"s","cases":[{"id":"a","category":"normal_answer","question":"q","expectation":{"kind":"evidence","required":["d"]},"origin":{"kind":"authored"}}]}"#;
        let defects = audit_case_set("set.json", text).unwrap_err();
        assert_eq!(tokens(&defects), vec!["field_missing"]);
        assert_eq!(defects[0].detail, "cases[0].expected");
    }

    #[test]
    fn an_expected_answer_with_nothing_to_check_is_refused() {
        let text = r#"{"case_set_schema":2,"scenario_version":"s","cases":[{"id":"a","category":"normal_answer","question":"q","expectation":{"kind":"evidence","required":["d"]},"expected":{"kind":"reference","must_include":[]},"origin":{"kind":"authored"}}]}"#;
        let defects = audit_case_set("set.json", text).unwrap_err();
        assert_eq!(tokens(&defects), vec!["field_empty"]);
        assert_eq!(
            defects[0].detail,
            "cases[0].expected.must_include must not be empty"
        );
    }

    #[test]
    fn an_unknown_expected_kind_and_an_unknown_key_are_refused() {
        let kind = r#"{"case_set_schema":2,"scenario_version":"s","cases":[{"id":"a","category":"normal_answer","question":"q","expectation":{"kind":"evidence","required":["d"]},"expected":{"kind":"semantic","must_include":["ok"]},"origin":{"kind":"authored"}}]}"#;
        assert_eq!(
            tokens(&audit_case_set("set.json", kind).unwrap_err()),
            vec!["expected_kind_unknown"]
        );

        let key = r#"{"case_set_schema":2,"scenario_version":"s","cases":[{"id":"a","category":"normal_answer","question":"q","expectation":{"kind":"evidence","required":["d"]},"expected":{"kind":"reference","must_include":["ok"],"similarity":0.8},"origin":{"kind":"authored"}}]}"#;
        assert_eq!(
            tokens(&audit_case_set("set.json", key).unwrap_err()),
            vec!["unknown_field"]
        );
    }

    #[test]
    fn must_exclude_may_be_absent_but_not_empty() {
        let absent = r#"{"case_set_schema":2,"scenario_version":"s","cases":[{"id":"a","category":"normal_answer","question":"q","expectation":{"kind":"evidence","required":["d"]},"expected":{"kind":"reference","must_include":["ok"]},"origin":{"kind":"authored"}}]}"#;
        let audit = audit_case_set("set.json", absent).unwrap();
        assert_eq!(audit.cases, 1);

        let empty = r#"{"case_set_schema":2,"scenario_version":"s","cases":[{"id":"a","category":"normal_answer","question":"q","expectation":{"kind":"evidence","required":["d"]},"expected":{"kind":"reference","must_include":["ok"],"must_exclude":[]},"origin":{"kind":"authored"}}]}"#;
        let defects = audit_case_set("set.json", empty).unwrap_err();
        assert_eq!(tokens(&defects), vec!["field_empty"]);
        assert_eq!(
            defects[0].detail,
            "cases[0].expected.must_exclude must not be empty"
        );
    }

    #[test]
    fn unknown_category_and_unknown_expectation_are_refused() {
        let category = r#"{"case_set_schema":2,"scenario_version":"s","cases":[{"id":"a","category":"gossip","question":"q","expectation":{"kind":"must_cancel"},"expected":{"kind":"reference","must_include":["ok"]},"origin":{"kind":"authored"}}]}"#;
        assert_eq!(
            tokens(&audit_case_set("set.json", category).unwrap_err()),
            vec!["category_unknown"]
        );

        let expectation = r#"{"case_set_schema":2,"scenario_version":"s","cases":[{"id":"a","category":"normal_answer","question":"q","expectation":{"kind":"maybe"},"expected":{"kind":"reference","must_include":["ok"]},"origin":{"kind":"authored"}}]}"#;
        assert_eq!(
            tokens(&audit_case_set("set.json", expectation).unwrap_err()),
            vec!["expectation_kind_unknown"]
        );
    }

    #[test]
    fn an_empty_evidence_list_is_a_broken_case_not_a_failing_system() {
        let text = r#"{"case_set_schema":2,"scenario_version":"s","cases":[{"id":"a","category":"normal_answer","question":"q","expectation":{"kind":"evidence","required":[]},"expected":{"kind":"reference","must_include":["ok"]},"origin":{"kind":"authored"}}]}"#;
        let defects = audit_case_set("set.json", text).unwrap_err();
        assert_eq!(tokens(&defects), vec!["field_empty"]);
        assert_eq!(
            defects[0].detail,
            "cases[0].expectation.required must not be empty"
        );
    }

    #[test]
    fn an_empty_forbidden_list_is_refused_too() {
        let text = r#"{"case_set_schema":2,"scenario_version":"s","cases":[{"id":"a","category":"fact_update","question":"q","expectation":{"kind":"high_risk_fact","forbidden_claims":[]},"expected":{"kind":"reference","must_include":["ok"]},"origin":{"kind":"authored"}}]}"#;
        let defects = audit_case_set("set.json", text).unwrap_err();
        assert_eq!(tokens(&defects), vec!["field_empty"]);
    }

    #[test]
    fn a_promoted_case_must_name_its_confirmer() {
        let missing = r#"{"case_set_schema":2,"scenario_version":"s","cases":[{"id":"a","category":"no_answer","question":"q","expectation":{"kind":"must_clarify_or_escalate"},"expected":{"kind":"reference","must_include":["ok"]},"origin":{"kind":"promoted"}}]}"#;
        let defects = audit_case_set("set.json", missing).unwrap_err();
        assert_eq!(defects[0].detail, "cases[0].origin.confirmer");

        let blank = r#"{"case_set_schema":2,"scenario_version":"s","cases":[{"id":"a","category":"no_answer","question":"q","expectation":{"kind":"must_clarify_or_escalate"},"expected":{"kind":"reference","must_include":["ok"]},"origin":{"kind":"promoted","confirmer":"  "}}]}"#;
        assert_eq!(
            tokens(&audit_case_set("set.json", blank).unwrap_err()),
            vec!["field_empty"]
        );

        let named = r#"{"case_set_schema":2,"scenario_version":"s","cases":[{"id":"a","category":"no_answer","question":"q","expectation":{"kind":"must_clarify_or_escalate"},"expected":{"kind":"reference","must_include":["ok"]},"origin":{"kind":"promoted","confirmer":"ops-zhang"}}]}"#;
        assert!(audit_case_set("set.json", named).is_ok());
    }

    #[test]
    fn a_duplicate_case_id_is_refused() {
        let case = r#"{"id":"a","category":"no_answer","question":"q","expectation":{"kind":"must_clarify_or_escalate"},"expected":{"kind":"reference","must_include":["ok"]},"origin":{"kind":"authored"}}"#;
        let text =
            format!(r#"{{"case_set_schema":2,"scenario_version":"s","cases":[{case},{case}]}}"#);
        let defects = audit_case_set("set.json", &text).unwrap_err();
        assert_eq!(tokens(&defects), vec!["duplicate_case_id"]);
        assert_eq!(defects[0].detail, "a");
    }

    #[test]
    fn an_absent_payload_is_reported_as_unsupplied_not_as_a_pass_or_a_failure() {
        let path = temp_dir("absent");
        let audit = audit_directory(&path).unwrap();
        assert!(audit.is_unsupplied());
        assert!(!audit.passed());
        assert_eq!(audit.files, 0);

        let error = check_dir(&path, "tests/rag-golden").unwrap_err();
        assert!(error.starts_with("CAPABILITY_NOT_READY"));
        assert!(error.contains("tests/rag-golden"));
    }

    #[test]
    fn only_json_files_are_read_and_in_name_order() {
        let root = temp_dir("scan");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("b.json"), set_json("scenario-b", 8)).unwrap();
        fs::write(root.join("a.json"), set_json("scenario-a", 8)).unwrap();
        fs::write(root.join("notes.md"), "not a case set").unwrap();

        let audit = audit_directory(&root).unwrap();
        assert_eq!(audit.files, 2);
        assert_eq!(audit.sets.len(), 2);
        assert_eq!(audit.sets[0].file, "a.json");
        assert_eq!(audit.sets[1].file, "b.json");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_qualifying_payload_passes_and_a_short_one_does_not() {
        let root = temp_dir("pass");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("set.json"), set_json("scenario-1", 100)).unwrap();
        assert!(audit_directory(&root).unwrap().passed());
        assert!(check_dir(&root, "fixture").is_ok());

        fs::write(root.join("set.json"), set_json("scenario-1", 8)).unwrap();
        let error = check_dir(&root, "fixture").unwrap_err();
        assert!(error.contains("structural rules"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_defective_payload_is_reported_as_a_defect_not_as_a_shortfall() {
        let root = temp_dir("defect");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("set.json"), "{").unwrap();
        let audit = audit_directory(&root).unwrap();
        assert_eq!(audit.files, 1);
        assert!(audit.sets.is_empty());
        assert!(!audit.passed());

        let error = check_dir(&root, "fixture").unwrap_err();
        assert!(error.contains("structural defect"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_unsupported_schema_revision_is_refused_by_the_directory_scan() {
        let root = temp_dir("revision");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("set.json"),
            r#"{"case_set_schema":9,"scenario_version":"s","cases":[]}"#,
        )
        .unwrap();
        let audit = audit_directory(&root).unwrap();
        assert_eq!(audit.files, 1);
        assert!(
            audit
                .defects
                .iter()
                .any(|defect| defect.token == "case_set_schema_unsupported")
        );
        let _ = fs::remove_dir_all(&root);
    }
}
