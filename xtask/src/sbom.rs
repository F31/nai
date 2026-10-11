//! Software bill of materials for the locked Rust dependency closure.
//!
//! The design asks for an SBOM per release (§11.6, §29.5) and names the SBOM as
//! a control for the supply-chain threat T-08, but it never says who produces it
//! or in what format. ADR 0043 settles that: this module renders a CycloneDX 1.5
//! document from the *same* `cargo metadata` view `check-deps` already uses,
//! plus the SHA-256 checksums `Cargo.lock` records. No new dependency is needed,
//! and no network access beyond what cargo resolution already requires.
//!
//! Two properties are deliberate and load-bearing:
//!
//! * **Determinism.** The same lock file produces byte-identical output, so a
//!   diff between two SBOMs means a real change in the dependency closure. That
//!   is why `timestamp` and `serialNumber` — both optional in CycloneDX — are
//!   left out: either one would change on every run.
//! * **No invented identifiers.** A declared license string is written as an
//!   SPDX `id` or `expression` only when every token is an identifier this
//!   module is willing to assert. Anything else is recorded verbatim as a
//!   `name`. CycloneDX keeps `license.id` and `license.name` in different
//!   fields, so the distinction survives into the artifact rather than being
//!   resolved silently on the upstream project's behalf.
//!
//! What this does **not** do is stated in the artifact itself, because an SBOM
//! that travels without its scope invites over-reading: there is no
//! vulnerability data, no signature, no dependency graph, and the frontend,
//! native binaries, models and media assets are outside the closure.

use std::{collections::BTreeMap, fs, path::Path};

use cargo_metadata::{MetadataCommand, Package};
use serde_json::{Map, Value, json};

/// SPDX identifiers this module is willing to assert.
///
/// Deliberately an explicit list rather than a shape test. A token that merely
/// *looks* like an identifier (say `BSD-2-Clause-FreeBSD`, which SPDX later
/// deprecated in favour of `BSD-2-Clause-Views`) would then be asserted with a
/// meaning we never checked. Recording it as a `name` understates; asserting it
/// as an `id` could state something false. Understatement wins.
const SPDX_IDS: &[&str] = &[
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "CC0-1.0",
    "CDLA-Permissive-2.0",
    "ISC",
    "LGPL-2.1-or-later",
    "LLVM-exception",
    "MIT",
    "MIT-0",
    "MPL-2.0",
    "Unicode-3.0",
    "Unlicense",
    "Zlib",
];

/// The only operators SPDX defines. A `/` is *not* one of them, which is why
/// the legacy `MIT/Apache-2.0` spelling cannot be passed through as an
/// expression.
const SPDX_OPERATORS: &[&str] = &["AND", "OR", "WITH"];

/// How a declared license string ended up in the output.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LicenseForm {
    /// A single SPDX identifier, asserted as such.
    Id,
    /// An SPDX expression whose every token is asserted.
    Expression,
    /// Recorded verbatim as a name: SPDX cannot express it, or it uses tokens
    /// this module does not assert.
    Name,
}

/// One component, flattened from `cargo metadata` and the lock file.
struct Component {
    name: String,
    version: String,
    /// The manifest's declared license, exactly as written.
    license: Option<String>,
    repository: Option<String>,
    /// True when the package builds a binary target.
    binary: bool,
    /// SHA-256 from `Cargo.lock`, present for registry packages only.
    checksum: Option<String>,
}

/// How the last render classified licenses. Printed so a new dependency with an
/// unfamiliar license string is *visible* rather than silently filed as a name.
#[derive(Default)]
struct Summary {
    total: usize,
    spdx_id: usize,
    expression: usize,
    verbatim: usize,
    no_license: usize,
    hashed: usize,
}

/// Writes the SBOM to `out`, or to standard output when it is `None`.
pub fn generate(root: &Path, out: Option<&Path>) -> Result<(), String> {
    let metadata = MetadataCommand::new()
        .current_dir(root)
        .other_options(vec!["--locked".into()])
        .exec()
        .map_err(|error| format!("cargo metadata failed: {error}"))?;
    let lock_path = root.join("Cargo.lock");
    let lock = fs::read_to_string(&lock_path)
        .map_err(|error| format!("cannot read {}: {error}", lock_path.display()))?;
    let checksums = lock_checksums(&lock)?;
    let components = components(&metadata.packages, &checksums)?;
    let (document, summary) = render(&components, env!("CARGO_PKG_VERSION"));
    match out {
        Some(path) => {
            fs::write(path, &document)
                .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
            eprintln!("sbom: wrote {}", path.display());
        }
        None => print!("{document}"),
    }
    eprintln!(
        "sbom: {} components — spdx-id {}, spdx-expression {}, verbatim-name {}, no-license {}; {} carry a sha256",
        summary.total,
        summary.spdx_id,
        summary.expression,
        summary.verbatim,
        summary.no_license,
        summary.hashed,
    );
    Ok(())
}

/// Flattens every locked package, refusing to drop one on the way.
///
/// The lock file and the metadata are two independent views of the same
/// resolution. A checksum with no matching package would mean the two have
/// drifted, and an SBOM built from only the metadata half would look complete
/// while quietly missing a package — so this is an error, not a warning.
fn components(
    packages: &[Package],
    checksums: &BTreeMap<(String, String), String>,
) -> Result<Vec<Component>, String> {
    let components: Vec<Component> = packages
        .iter()
        .map(|package| {
            let name = package.name.to_string();
            let version = package.version.to_string();
            Component {
                repository: package.repository.clone().filter(|url| !url.is_empty()),
                binary: package.targets.iter().any(|target| target.is_bin()),
                checksum: checksums.get(&(name.clone(), version.clone())).cloned(),
                name,
                version,
                license: package.license.clone(),
            }
        })
        .collect();
    for key in checksums.keys() {
        if !components
            .iter()
            .any(|component| component.name == key.0 && component.version == key.1)
        {
            return Err(format!(
                "Cargo.lock lists {} {} but cargo metadata does not: \
                 refusing to emit an SBOM that silently omits a locked package",
                key.0, key.1
            ));
        }
    }
    Ok(components)
}

/// Renders the CycloneDX document. Sorting happens here, not upstream, so the
/// output cannot depend on the order `cargo metadata` happens to return.
fn render(components: &[Component], harness_version: &str) -> (String, Summary) {
    let mut sorted: Vec<&Component> = components.iter().collect();
    sorted.sort_by(|left, right| {
        (left.name.as_str(), left.version.as_str())
            .cmp(&(right.name.as_str(), right.version.as_str()))
    });

    let mut summary = Summary::default();
    let mut list = Vec::with_capacity(sorted.len());
    for component in sorted {
        let purl = format!("pkg:cargo/{}@{}", component.name, component.version);
        let mut entry = Map::new();
        entry.insert(
            "type".into(),
            json!(if component.binary {
                "application"
            } else {
                "library"
            }),
        );
        entry.insert("bom-ref".into(), json!(purl));
        entry.insert("name".into(), json!(component.name));
        entry.insert("version".into(), json!(component.version));
        entry.insert("purl".into(), json!(purl));
        if let Some(repository) = &component.repository {
            entry.insert(
                "externalReferences".into(),
                json!([{"type": "vcs", "url": repository}]),
            );
        }
        if let Some(checksum) = &component.checksum {
            summary.hashed += 1;
            entry.insert(
                "hashes".into(),
                json!([{"alg": "SHA-256", "content": checksum}]),
            );
        }
        // Every component gets a licenses entry. Omitting the field for an
        // unlicensed manifest would make "no license declared" and "we lost the
        // license" look identical from the outside.
        match &component.license {
            Some(raw) => {
                let licenses = match classify(raw) {
                    LicenseForm::Id => {
                        summary.spdx_id += 1;
                        json!([{"license": {"id": raw}}])
                    }
                    LicenseForm::Expression => {
                        summary.expression += 1;
                        json!([{"expression": raw}])
                    }
                    LicenseForm::Name => {
                        summary.verbatim += 1;
                        json!([{"license": {"name": raw}}])
                    }
                };
                entry.insert("licenses".into(), licenses);
            }
            None => {
                summary.no_license += 1;
                entry.insert(
                    "licenses".into(),
                    json!([{"license": {"name": "NOASSERTION"}}]),
                );
            }
        }
        list.push(Value::Object(entry));
    }
    summary.total = list.len();

    let document = json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "component": {"type": "application", "name": "nai", "version": harness_version},
            "tools": [{
                "vendor": "nai",
                "name": env!("CARGO_PKG_NAME"),
                "version": harness_version,
            }],
            "properties": [
                {
                    "name": "nai:sbom:scope",
                    "value": "the Rust dependency closure fixed by Cargo.lock, \
                              including every workspace member",
                },
                {
                    "name": "nai:sbom:excluded",
                    "value": "frontend npm dependencies; native binaries (ONNX Runtime, CUDA, \
                              cuDNN, media codecs); models, media assets and portrait/voice \
                              licences; per-plugin package BOMs",
                },
                {
                    "name": "nai:sbom:not-evidence-for",
                    "value": "vulnerability status, signatures, or release acceptance — \
                              this artifact records what was locked and under which licence",
                },
            ],
        },
        "components": list,
    });
    let mut text = serde_json::to_string_pretty(&document)
        .expect("a document built from json! is always serialisable");
    text.push('\n');
    (text, summary)
}

/// Classifies a declared license string without reinterpreting it.
fn classify(raw: &str) -> LicenseForm {
    let trimmed = raw.trim();
    if SPDX_IDS.contains(&trimmed) {
        return LicenseForm::Id;
    }
    let tokens: Vec<&str> = trimmed
        .split(|character: char| character.is_whitespace() || character == '(' || character == ')')
        .filter(|token| !token.is_empty())
        .collect();
    // An expression alternates identifier/operator; `WITH` binds a licence to an
    // exception, which keeps the alternation. Requiring the alternation rejects
    // strings that merely list identifiers without relating them.
    let well_formed = !tokens.is_empty()
        && tokens.len() % 2 == 1
        && tokens.iter().enumerate().all(|(index, token)| {
            if index % 2 == 0 {
                SPDX_IDS.contains(token)
            } else {
                SPDX_OPERATORS.contains(token)
            }
        });
    if well_formed {
        LicenseForm::Expression
    } else {
        LicenseForm::Name
    }
}

/// Reads the SHA-256 checksums `Cargo.lock` records for registry packages.
///
/// Minimal on purpose. The lock file is written by cargo with a stable writer
/// and this reads exactly four keys out of each `[[package]]` block; anything
/// else in the file is skipped, but a registry package with no checksum, a
/// checksum that is not 64 lowercase hex digits, or a value that is not a quoted
/// string is an error. A component that silently loses its hash would make the
/// artifact look more complete than it is.
fn lock_checksums(text: &str) -> Result<BTreeMap<(String, String), String>, String> {
    let mut entries = BTreeMap::new();
    let mut blocks = 0usize;
    let mut current: Option<RawPackage> = None;
    for line in text.lines() {
        if line.trim() == "[[package]]" {
            finish(current.take(), &mut entries)?;
            blocks += 1;
            current = Some(RawPackage::default());
            continue;
        }
        let Some(package) = current.as_mut() else {
            continue;
        };
        // Keys inside a block start at column zero; array elements are indented,
        // which is what keeps `dependencies = [ ... ]` from being read as keys.
        if line.is_empty() || line.starts_with(|character: char| character.is_whitespace()) {
            continue;
        }
        let Some((key, value)) = line.split_once(" = ") else {
            continue;
        };
        // Match the key *before* reading the value: a block also holds
        // `dependencies = [ ... ]`, whose value is not a quoted string.
        let field = match key {
            "name" => &mut package.name,
            "version" => &mut package.version,
            "source" => &mut package.source,
            "checksum" => &mut package.checksum,
            _ => continue,
        };
        let value = value.trim();
        *field = Some(
            unquote(value)
                .ok_or_else(|| format!("Cargo.lock: {key} is not a quoted string: {value}"))?,
        );
    }
    finish(current.take(), &mut entries)?;
    if blocks == 0 {
        return Err("Cargo.lock has no [[package]] blocks; is it a lock file?".into());
    }
    Ok(entries)
}

#[derive(Default)]
struct RawPackage {
    name: Option<String>,
    version: Option<String>,
    source: Option<String>,
    checksum: Option<String>,
}

/// Closes a `[[package]]` block, keeping it only when it is an external package
/// whose hash we can state.
fn finish(
    package: Option<RawPackage>,
    entries: &mut BTreeMap<(String, String), String>,
) -> Result<(), String> {
    let Some(package) = package else {
        return Ok(());
    };
    let (Some(name), Some(version)) = (package.name, package.version) else {
        return Err("Cargo.lock: a [[package]] block is missing name or version".into());
    };
    let registry = package
        .source
        .as_deref()
        .is_some_and(|source| source.starts_with("registry+"));
    if !registry {
        // Workspace members and path dependencies carry no checksum by design.
        return Ok(());
    }
    let Some(checksum) = package.checksum else {
        return Err(format!(
            "Cargo.lock: {name} {version} comes from a registry but has no checksum"
        ));
    };
    if checksum.len() != 64
        || !checksum
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
    {
        return Err(format!(
            "Cargo.lock: {name} {version} has a checksum that is not 64 lowercase hex digits"
        ));
    }
    entries.insert((name, version), checksum);
    Ok(())
}

/// Returns the contents of a `"..."` value, or `None` when it is anything else.
fn unquote(value: &str) -> Option<String> {
    let inner = value.strip_prefix('"')?.strip_suffix('"')?;
    // Cargo's writer escapes nothing in the keys read here; an embedded quote
    // means this is not the shape we think it is.
    if inner.contains('"') {
        return None;
    }
    Some(inner.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component(name: &str, version: &str, license: Option<&str>) -> Component {
        Component {
            name: name.into(),
            version: version.into(),
            license: license.map(str::to_string),
            repository: Some(format!("https://example.invalid/{name}")),
            binary: false,
            checksum: None,
        }
    }

    #[test]
    fn a_single_known_identifier_is_asserted_as_an_id() {
        assert_eq!(classify("MIT"), LicenseForm::Id);
        assert_eq!(classify("Apache-2.0"), LicenseForm::Id);
        assert_eq!(classify("Unicode-3.0"), LicenseForm::Id);
    }

    #[test]
    fn expressions_are_asserted_only_when_every_token_is_known() {
        assert_eq!(classify("MIT OR Apache-2.0"), LicenseForm::Expression);
        assert_eq!(
            classify("Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT"),
            LicenseForm::Expression
        );
        assert_eq!(
            classify("(Apache-2.0 OR MIT) AND BSD-3-Clause"),
            LicenseForm::Expression
        );
        assert_eq!(
            classify("ISC AND (Apache-2.0 OR ISC)"),
            LicenseForm::Expression
        );
    }

    /// The four legacy spellings actually present in this workspace's lock file.
    /// A `/` is not an SPDX operator, so none of them may be passed through as an
    /// expression — and none may be silently rewritten into `OR`.
    #[test]
    fn legacy_slash_spellings_are_recorded_verbatim_not_reinterpreted() {
        for raw in [
            "Apache-2.0 / MIT",
            "Apache-2.0/MIT",
            "MIT/Apache-2.0",
            "Unlicense/MIT",
        ] {
            assert_eq!(classify(raw), LicenseForm::Name, "{raw}");
        }
    }

    #[test]
    fn malformed_or_unknown_expressions_fall_back_to_a_name() {
        assert_eq!(classify(""), LicenseForm::Name);
        assert_eq!(classify("OR MIT"), LicenseForm::Name);
        assert_eq!(classify("MIT OR"), LicenseForm::Name);
        assert_eq!(classify("MIT MIT"), LicenseForm::Name);
        assert_eq!(classify("MIT OR Unknown-1.0"), LicenseForm::Name);
    }

    #[test]
    fn render_records_a_license_entry_for_every_component() {
        let components = vec![
            component("alpha", "1.0.0", Some("MIT")),
            component("beta", "2.0.0", Some("MIT/Apache-2.0")),
            component("gamma", "3.0.0", None),
        ];
        let (document, summary) = render(&components, "0.0.0");
        assert_eq!(summary.total, 3);
        assert_eq!(summary.spdx_id, 1);
        assert_eq!(summary.verbatim, 1);
        assert_eq!(summary.no_license, 1);

        let value: Value = serde_json::from_str(&document).expect("valid json");
        let list = value["components"].as_array().expect("components");
        assert_eq!(list.len(), 3);
        for entry in list {
            assert!(
                entry["licenses"].as_array().is_some_and(|l| !l.is_empty()),
                "every component must carry a licenses entry: {entry}"
            );
        }
    }

    #[test]
    fn render_is_deterministic_and_order_independent() {
        let mut components = vec![
            component("zulu", "1.0.0", Some("MIT")),
            component("alpha", "2.0.0", Some("MIT OR Apache-2.0")),
            component("alpha", "1.0.0", Some("Zlib")),
        ];
        let (first, _) = render(&components, "0.0.0");
        components.reverse();
        let (second, _) = render(&components, "0.0.0");
        assert_eq!(first, second, "output must not depend on input order");

        let value: Value = serde_json::from_str(&first).expect("valid json");
        let names: Vec<(&str, &str)> = value["components"]
            .as_array()
            .expect("components")
            .iter()
            .map(|entry| {
                (
                    entry["name"].as_str().expect("name"),
                    entry["version"].as_str().expect("version"),
                )
            })
            .collect();
        assert_eq!(
            names,
            vec![("alpha", "1.0.0"), ("alpha", "2.0.0"), ("zulu", "1.0.0")]
        );
    }

    #[test]
    fn render_carries_its_own_scope_and_a_hash_when_the_lock_had_one() {
        let mut component = component("alpha", "1.0.0", Some("MIT"));
        component.checksum = Some("0".repeat(64));
        let (document, summary) = render(&[component], "0.0.0");
        assert_eq!(summary.hashed, 1);
        let value: Value = serde_json::from_str(&document).expect("valid json");
        assert_eq!(value["bomFormat"], "CycloneDX");
        assert_eq!(value["specVersion"], "1.5");
        let hashes = value["components"][0]["hashes"].as_array().expect("hashes");
        assert_eq!(hashes[0]["alg"], "SHA-256");
        let properties = value["metadata"]["properties"]
            .as_array()
            .expect("properties");
        let names: Vec<&str> = properties
            .iter()
            .map(|property| property["name"].as_str().expect("name"))
            .collect();
        assert!(names.contains(&"nai:sbom:scope"));
        assert!(names.contains(&"nai:sbom:not-evidence-for"));
    }

    #[test]
    fn lock_checksums_reads_the_real_lock_file() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask sits at the workspace root")
            .join("Cargo.lock");
        let text = fs::read_to_string(&path).expect("the workspace has a lock file");
        let checksums = lock_checksums(&text).expect("the real lock file parses");
        assert!(
            checksums.len() > 100,
            "expected the workspace's external packages, got {}",
            checksums.len()
        );
        // `serde_json` is pinned by the workspace and must be in the closure.
        let serde_json = checksums
            .iter()
            .find(|((name, _), _)| name == "serde_json")
            .expect("serde_json is a locked dependency");
        assert_eq!(serde_json.1.len(), 64);
        assert!(serde_json.1.starts_with(|c: char| c.is_ascii_hexdigit()));
    }

    #[test]
    fn lock_checksums_refuses_a_registry_package_without_a_checksum() {
        let text = "version = 4\n\n[[package]]\nname = \"x\"\nversion = \"1.0.0\"\n\
                    source = \"registry+https://github.com/rust-lang/crates.io-index\"\n";
        let error = lock_checksums(text).expect_err("a missing hash must not pass");
        assert!(error.contains("has no checksum"), "{error}");
    }

    #[test]
    fn lock_checksums_refuses_a_malformed_checksum() {
        let text = "[[package]]\nname = \"x\"\nversion = \"1.0.0\"\n\
                    source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\
                    checksum = \"NOTHEX\"\n";
        let error = lock_checksums(text).expect_err("a malformed hash must not pass");
        assert!(error.contains("64 lowercase hex"), "{error}");
    }

    #[test]
    fn lock_checksums_ignores_path_packages_and_indented_dependency_arrays() {
        let text = "version = 4\n\n[[package]]\nname = \"local\"\nversion = \"0.1.0\"\n\
                    dependencies = [\n \"serde_json\",\n]\n\n[[package]]\nname = \"serde_json\"\n\
                    version = \"1.0.151\"\n\
                    source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\
                    checksum = \"1111111111111111111111111111111111111111111111111111111111111111\"\n";
        let checksums = lock_checksums(text).expect("parses");
        assert_eq!(checksums.len(), 1);
        assert!(checksums.contains_key(&("serde_json".to_string(), "1.0.151".to_string())));
    }
}
