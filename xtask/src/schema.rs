use prost::Message;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const OUTPUTS: [(&str, &str); 3] = [
    ("rust", "crates/nai-contracts/src/generated"),
    ("ts", "packages/contracts-ts/src/generated"),
    ("cpp", "native/contracts/generated"),
];

fn files(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, String> {
    fn visit(
        base: &Path,
        dir: &Path,
        output: &mut BTreeMap<PathBuf, Vec<u8>>,
    ) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().map_err(|e| e.to_string())?.is_symlink() {
                return Err(format!(
                    "generated path must not be a symlink: {}",
                    entry.path().display()
                ));
            }
            if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                visit(base, &entry.path(), output)?;
            } else {
                output.insert(
                    entry
                        .path()
                        .strip_prefix(base)
                        .map_err(|e| e.to_string())?
                        .to_owned(),
                    fs::read(entry.path()).map_err(|e| e.to_string())?,
                );
            }
        }
        Ok(())
    }
    let mut result = BTreeMap::new();
    if root.exists() {
        visit(root, root, &mut result)?;
    }
    Ok(result)
}

fn differences(
    expected: &BTreeMap<PathBuf, Vec<u8>>,
    actual: &BTreeMap<PathBuf, Vec<u8>>,
) -> Vec<String> {
    expected
        .keys()
        .chain(actual.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|path| expected.get(*path) != actual.get(*path))
        .map(|path| format!("generated drift: {}", path.display()))
        .collect()
}

fn sync_output(source: &Path, destination: &Path, check: bool) -> Result<Vec<String>, String> {
    let generated = files(source)?;
    if check {
        return Ok(differences(&generated, &files(destination)?));
    }
    // The destination is exclusively generator-owned, never a consumer directory.
    if destination.exists() {
        fs::remove_dir_all(destination).map_err(|e| e.to_string())?;
    }
    for (path, contents) in generated {
        let path = destination.join(path);
        fs::create_dir_all(path.parent().ok_or("missing generated parent")?)
            .map_err(|e| e.to_string())?;
        fs::write(path, contents).map_err(|e| e.to_string())?;
    }
    Ok(Vec::new())
}

pub fn node(root: &Path, args: &[&str]) -> Result<(), String> {
    let status = Command::new("node")
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|e| format!("node unavailable: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("node {} failed: {status}", args.join(" ")))
    }
}

fn descriptor_names(
    messages: &[prost_types::DescriptorProto],
    prefix: &str,
    names: &mut BTreeSet<String>,
) {
    for message in messages {
        let name = format!("{prefix}.{}", message.name.as_deref().unwrap_or_default());
        names.insert(name.clone());
        descriptor_names(&message.nested_type, &name, names);
    }
}

fn validate_catalog(root: &Path, descriptor: &Path) -> Result<Vec<String>, String> {
    let bytes = fs::read(descriptor).map_err(|e| e.to_string())?;
    let set =
        prost_types::FileDescriptorSet::decode(bytes.as_slice()).map_err(|e| e.to_string())?;
    let mut messages = BTreeSet::new();
    for file in &set.file {
        descriptor_names(
            &file.message_type,
            file.package.as_deref().unwrap_or_default(),
            &mut messages,
        );
    }
    let catalog: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("schemas/catalog/capabilities.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    catalog_types(&catalog, &messages)
}

fn catalog_types(
    catalog: &serde_json::Value,
    messages: &BTreeSet<String>,
) -> Result<Vec<String>, String> {
    let mut ids = BTreeSet::new();
    let mut types = BTreeSet::new();
    let mut frozen = 0usize;
    let mut drafts = 0usize;
    for capability in catalog["capabilities"]
        .as_array()
        .ok_or("capabilities must be an array")?
    {
        let id = capability["id"].as_str().ok_or("missing capability id")?;
        if !ids.insert(id) {
            return Err(format!("duplicate capability: {id}"));
        }
        let stage = capability["stage"]
            .as_str()
            .ok_or("missing capability stage")?;
        if stage == "draft" {
            // A draft has no frozen wire representation and generates no
            // consumer code. Declaring part of one -- a request without a
            // response, or a stream flag -- would make the stage field
            // unreadable, so a draft that declares any of it is an error rather
            // than something to tolerate.
            for key in ["request", "response", "stream", "session_scoped"] {
                if !capability[key].is_null() {
                    return Err(format!("draft capability {id} declares {key}"));
                }
            }
            drafts += 1;
            continue;
        }
        if stage != "frozen" {
            return Err(format!("{id} has unknown capability stage {stage}"));
        }
        for key in ["request", "response"] {
            let name = capability[key].as_str().ok_or("missing message type")?;
            if !messages.contains(name) {
                return Err(format!("{id} references absent message {name}"));
            }
            types.insert(name.to_owned());
        }
        frozen += 1;
    }
    for adapter in catalog["adapters_required"]
        .as_array()
        .ok_or("missing adapter list")?
    {
        if !ids.contains(adapter["target"].as_str().ok_or("missing adapter target")?) {
            return Err("legacy adapter references absent target".into());
        }
    }
    for name in [
        "Handshake",
        "HandshakeAck",
        "CancelRequest",
        "CancelAck",
        "CapabilityInvocation",
        "CapabilityResult",
        "ContractError",
        "HealthRequest",
        "HealthResponse",
    ] {
        let name = format!("nai.v1.{name}");
        if !messages.contains(&name) {
            return Err(format!("missing core message {name}"));
        }
        types.insert(name);
    }
    println!(
        "catalog checked: {} capabilities ({} frozen, {} draft); {} protobuf messages",
        ids.len(),
        frozen,
        drafts,
        messages.len()
    );
    Ok(types.into_iter().collect())
}

pub fn generate(root: &Path, check: bool) -> Result<(), String> {
    let protoc = std::env::var_os("PROTOC").unwrap_or_else(|| "protoc".into());
    let version = Command::new(&protoc)
        .arg("--version")
        .output()
        .map_err(|e| format!("install protoc 3.21.12: {e}"))?;
    if !version.status.success()
        || String::from_utf8_lossy(&version.stdout).trim() != "libprotoc 3.21.12"
    {
        return Err("generation requires exactly protoc 3.21.12".into());
    }
    let stage = root.join(format!("target/schema-stage-{}", std::process::id()));
    if stage.exists() {
        fs::remove_dir_all(&stage).map_err(|e| e.to_string())?;
    }
    for (name, _) in OUTPUTS {
        fs::create_dir_all(stage.join(name)).map_err(|e| e.to_string())?;
    }
    let protos: Vec<_> = files(&root.join("schemas"))?
        .keys()
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "proto")
        })
        .map(|path| root.join("schemas").join(path))
        .collect();
    if protos.is_empty() {
        return Err("no protobuf sources found".into());
    }
    let descriptor = stage.join("descriptor.bin");
    let mut config = prost_build::Config::new();
    config
        .out_dir(stage.join("rust"))
        .btree_map(["."])
        // Slot commit embeds ticket+frame metadata; box the oneof variant
        // without changing the protobuf wire representation or editing Rust.
        .boxed(".nai.v1.FrameSlotEvent.event.committed")
        .file_descriptor_set_path(&descriptor);
    config
        .compile_protos(&protos, &[root.join("schemas")])
        .map_err(|e| format!("prost generation failed: {e}"))?;
    let types = validate_catalog(root, &descriptor)?;
    let catalog: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("schemas/catalog/capabilities.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let capabilities = catalog["capabilities"]
        .as_array()
        .ok_or("missing capabilities")?;
    let mut session_types = BTreeSet::new();
    let mut capability_ids = BTreeSet::new();
    let mut draft_capability_ids = BTreeSet::new();
    for capability in capabilities {
        let id = capability["id"]
            .as_str()
            .ok_or("missing capability id")?
            .to_owned();
        match capability["stage"].as_str() {
            // Only frozen entries generate consumer code: the negotiation list
            // and the message-type tables must stay congruent with what has
            // actually been frozen.
            Some("frozen") => {
                if capability["session_scoped"].as_bool() == Some(true) {
                    for key in ["request", "response"] {
                        session_types.insert(
                            capability[key]
                                .as_str()
                                .ok_or("missing message type")?
                                .to_owned(),
                        );
                    }
                }
                capability_ids.insert(id);
            }
            Some("draft") => {
                draft_capability_ids.insert(id);
            }
            _ => return Err("capability stage must be draft or frozen".into()),
        }
    }
    let session_types: Vec<_> = session_types.into_iter().collect();
    let capability_ids: Vec<_> = capability_ids.into_iter().collect();
    let draft_capability_ids: Vec<_> = draft_capability_ids.into_iter().collect();
    fs::write(stage.join("rust/catalog.rs"), format!("// Generated from schemas/catalog/capabilities.json. Do not edit.\npub const MESSAGE_TYPES: &[&str] = &{types:#?};\npub const SESSION_MESSAGE_TYPES: &[&str] = &{session_types:#?};\npub const CAPABILITY_IDS: &[&str] = &{capability_ids:#?};\n/// Declared but not frozen: listed so the catalog is judgeable, never negotiable.\npub const DRAFT_CAPABILITY_IDS: &[&str] = &{draft_capability_ids:#?};\n")).map_err(|e| e.to_string())?;
    let status = Command::new(&protoc)
        .arg(format!("-I{}", root.join("schemas").display()))
        .arg(format!("--cpp_out={}", stage.join("cpp").display()))
        .args(&protos)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("C++ generation failed".into());
    }
    // Preserve proto2 optional field presence, notably oneof_index = Some(0).
    // Re-encoding a descriptor with JS codecs that use scalar defaults would
    // silently erase the first oneof and change the generated TS contract.
    let set = prost_types::FileDescriptorSet::decode(
        fs::read(&descriptor).map_err(|e| e.to_string())?.as_slice(),
    )
    .map_err(|e| e.to_string())?;
    let request = prost_types::compiler::CodeGeneratorRequest {
        file_to_generate: set.file.iter().map(|file| file.name.clone().unwrap_or_default()).collect(),
        parameter: Some("forceLong=string,oneof=unions,esModuleInterop=true,outputJsonMethods=false,outputServices=none,importSuffix=.js,useExactTypes=false".into()),
        proto_file: set.file,
        compiler_version: Some(prost_types::compiler::Version { major: Some(3), minor: Some(21), patch: Some(12), suffix: None }),
    };
    fs::write(stage.join("request.bin"), request.encode_to_vec()).map_err(|e| e.to_string())?;
    node(
        root,
        &[
            "scripts/contracts/generate.mjs",
            stage.to_str().ok_or("invalid stage path")?,
        ],
    )?;
    let mut drift = Vec::new();
    for (name, destination) in OUTPUTS {
        let destination = root.join(destination);
        drift.extend(
            sync_output(&stage.join(name), &destination, check)?
                .into_iter()
                .map(|error| format!("{}: {error}", destination.display())),
        );
    }
    fs::create_dir_all(root.join("target/contracts")).map_err(|e| e.to_string())?;
    fs::copy(&descriptor, root.join("target/contracts/descriptor.bin"))
        .map_err(|e| e.to_string())?;
    fs::remove_dir_all(stage).map_err(|e| e.to_string())?;
    if !drift.is_empty() {
        return Err(drift.join("\n"));
    }
    println!(
        "Rust/TS/C++/JSON Schema/OpenAPI generation {}",
        if check {
            "checked with no drift"
        } else {
            "completed"
        }
    );
    Ok(())
}

fn program(root: &Path, executable: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(executable)
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|e| format!("{executable} unavailable: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{executable} failed: {status}"))
    }
}

pub fn contracts(root: &Path) -> Result<(), String> {
    generate(root, true)?;
    super::run(root, &["test", "--package", "nai-contracts", "--locked"])?;
    node(
        root,
        &[
            "packages/contracts-ts/node_modules/typescript/bin/tsc",
            "-p",
            "packages/contracts-ts/tsconfig.json",
        ],
    )?;
    node(
        root,
        &[
            "--test",
            "tests/contracts/schema.test.mjs",
            "tests/contracts/descriptor.test.mjs",
        ],
    )?;
    program(
        root,
        "cmake",
        &["-S", "tests/contracts/cpp", "-B", "target/contracts/cpp"],
    )?;
    program(
        root,
        "cmake",
        &[
            "--build",
            "target/contracts/cpp",
            "--config",
            "Release",
            "--parallel",
            "2",
        ],
    )?;
    let executable = if cfg!(target_os = "windows") {
        "target/contracts/cpp/Release/contract-roundtrip.exe"
    } else {
        "target/contracts/cpp/contract-roundtrip"
    };
    for directory in ["rust", "ts", "cpp-output", "cpp-future", "ts-future"] {
        fs::create_dir_all(root.join("target/contracts").join(directory))
            .map_err(|e| e.to_string())?;
    }
    super::run(
        root,
        &[
            "run",
            "--locked",
            "--package",
            "nai-contracts",
            "--example",
            "contract_fixture",
            "--",
            "write",
            "target/contracts/rust",
        ],
    )?;
    node(
        root,
        &[
            "tests/contracts/interop.mjs",
            "target/contracts/rust",
            "target/contracts/ts",
        ],
    )?;
    program(
        root,
        executable,
        &["target/contracts/ts", "target/contracts/cpp-output"],
    )?;
    super::run(
        root,
        &[
            "run",
            "--locked",
            "--package",
            "nai-contracts",
            "--example",
            "contract_fixture",
            "--",
            "verify",
            "target/contracts/cpp-output",
        ],
    )?;
    program(
        root,
        executable,
        &["target/contracts/rust", "target/contracts/cpp-future"],
    )?;
    node(
        root,
        &[
            "tests/contracts/interop.mjs",
            "target/contracts/cpp-future",
            "target/contracts/ts-future",
        ],
    )?;
    super::run(
        root,
        &[
            "run",
            "--locked",
            "--package",
            "nai-contracts",
            "--example",
            "contract_fixture",
            "--",
            "verify",
            "target/contracts/ts-future",
        ],
    )?;
    println!(
        "contracts suite passed: Rust/TS/C++ compilation, schema/API validation and both cross-language paths"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_modified_missing_and_stale_generated_files_without_mutation() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("nai-generation-{}-{nonce}", std::process::id()));
        let expected = root.join("source");
        let actual = root.join("consumer");
        fs::create_dir_all(&expected).unwrap();
        fs::create_dir_all(&actual).unwrap();
        fs::write(expected.join("message.rs"), [1]).unwrap();
        fs::write(expected.join("missing.rs"), [2]).unwrap();
        fs::write(actual.join("message.rs"), [9]).unwrap();
        fs::write(actual.join("stale.rs"), [3]).unwrap();
        let before = files(&actual).unwrap();
        let errors = sync_output(&expected, &actual, true).unwrap();
        assert_eq!(errors.len(), 3);
        assert_eq!(
            files(&actual).unwrap(),
            before,
            "--check must not repair a consumer"
        );
        sync_output(&expected, &actual, false).unwrap();
        assert!(sync_output(&expected, &actual, true).unwrap().is_empty());
        assert!(!actual.join("stale.rs").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn catalog_rejects_duplicate_unresolved_and_half_declared_entries() {
        let catalog = serde_json::json!({"capabilities":[
            {"id":"test.invoke@1","stage":"frozen","request":"nai.v1.Handshake","response":"nai.v1.HandshakeAck"},
            {"id":"test.draft@1","stage":"draft"}
        ],"adapters_required":[]});
        let names = [
            "Handshake",
            "HandshakeAck",
            "CancelRequest",
            "CancelAck",
            "CapabilityInvocation",
            "CapabilityResult",
            "ContractError",
            "HealthRequest",
            "HealthResponse",
        ];
        let mut messages: BTreeSet<_> = names
            .map(|name| format!("nai.v1.{name}"))
            .into_iter()
            .collect();
        assert!(catalog_types(&catalog, &messages).is_ok());
        let mut duplicated = catalog.clone();
        duplicated["capabilities"]
            .as_array_mut()
            .unwrap()
            .push(catalog["capabilities"][0].clone());
        assert!(
            catalog_types(&duplicated, &messages)
                .unwrap_err()
                .contains("duplicate")
        );
        messages.remove("nai.v1.HandshakeAck");
        assert!(
            catalog_types(&catalog, &messages)
                .unwrap_err()
                .contains("absent message")
        );
        messages.insert("nai.v1.HandshakeAck".into());
        // A draft that declares any consumer field is an error rather than a
        // warning: half-declaring makes the stage field unreadable, and the
        // whole point of the field is that it decides the generated surface.
        let mut half = catalog.clone();
        half["capabilities"][1]["request"] = serde_json::json!("nai.v1.Handshake");
        assert!(
            catalog_types(&half, &messages)
                .unwrap_err()
                .contains("declares request")
        );
        let mut unknown = catalog.clone();
        unknown["capabilities"][1]["stage"] = serde_json::json!("deprecated");
        assert!(
            catalog_types(&unknown, &messages)
                .unwrap_err()
                .contains("unknown capability stage")
        );
    }
}
