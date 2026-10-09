use cargo_metadata::MetadataCommand;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

#[derive(Debug)]
struct Package {
    name: String,
    path: String,
    layer: String,
    // Includes normal, build, dev and target-specific dependencies, using their
    // real package names rather than Rust import aliases.
    dependencies: Vec<String>,
}

fn expected_layer(path: &str) -> Option<&'static str> {
    match path {
        "xtask" => Some("tool"),
        "crates/nai-contracts" => Some("contracts"),
        "crates/nai-kernel" => Some("kernel"),
        "crates/nai-plugin-sdk" => Some("sdk"),
        "crates/nai-runtime" => Some("runtime"),
        "crates/nai-plugin-hosts" => Some("hosts"),
        "crates/nai-media-core" => Some("media"),
        "crates/nai-provider-core" => Some("provider"),
        "crates/nai-storage"
        | "crates/nai-budget-core"
        | "crates/nai-security"
        | "crates/nai-knowledge-core"
        | "crates/nai-control-api"
        | "crates/nai-upgrade-core"
        | "crates/nai-conformance" => Some("shared"),
        "apps/media-worker" | "apps/output-keeper" | "apps/render-worker" => Some("media"),
        path if path.starts_with("plugins/") => Some("plugin"),
        path if path.starts_with("apps/") => Some("app"),
        _ => None,
    }
}

fn reachable<'a>(source: &'a Package, packages: &'a HashMap<&str, &Package>) -> Vec<&'a Package> {
    let mut pending = source.dependencies.clone();
    let mut visited = HashSet::new();
    let mut result = Vec::new();
    while let Some(name) = pending.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        if let Some(target) = packages.get(name.as_str()) {
            result.push(*target);
            pending.extend(target.dependencies.iter().cloned());
        }
    }
    result
}

fn validate(packages: &[Package]) -> Vec<String> {
    let by_name: HashMap<_, _> = packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect();
    let mut errors = Vec::new();
    for source in packages {
        if expected_layer(&source.path) != Some(source.layer.as_str()) {
            errors.push(format!(
                "{}: path {} does not match declared layer {}",
                source.name, source.path, source.layer
            ));
        }
        for target in reachable(source, &by_name) {
            let forbidden = match source.layer.as_str() {
                "contracts" => true,
                "kernel" | "sdk" => target.layer != "contracts",
                "shared" | "provider" => {
                    matches!(target.layer.as_str(), "plugin" | "runtime" | "app" | "tool")
                }
                "hosts" => matches!(target.layer.as_str(), "plugin" | "runtime" | "app"),
                "plugin" => matches!(target.layer.as_str(), "plugin" | "runtime" | "app"),
                "media" => {
                    target.layer == "provider"
                        || target.path.starts_with("plugins/llm-")
                        || target.path.starts_with("plugins/tts-")
                        || target.path.starts_with("plugins/gateway-")
                }
                _ => false,
            };
            if forbidden {
                errors.push(format!(
                    "{} ({}) must not depend on {} ({})",
                    source.name, source.layer, target.name, target.layer
                ));
            }
        }
        // Inspect external names at every reachable workspace node as well.
        for node in std::iter::once(source).chain(reachable(source, &by_name)) {
            for dependency in &node.dependencies {
                let native = dependency.starts_with("tauri")
                    || dependency.starts_with("windows")
                    || dependency.starts_with("douyin")
                    || dependency == "ort"
                    || dependency.starts_with("cuda");
                let http = matches!(dependency.as_str(), "reqwest" | "hyper" | "ureq");
                if (matches!(source.layer.as_str(), "contracts" | "kernel") && native)
                    || (source.layer == "media" && http)
                {
                    errors.push(format!(
                        "{} ({}) must not access dependency {} via {}",
                        source.name, source.layer, dependency, node.name
                    ));
                }
            }
        }
    }
    errors.sort();
    errors.dedup();
    errors
}

pub fn check(root: &Path) -> Result<(), String> {
    let metadata = MetadataCommand::new()
        .current_dir(root)
        .no_deps()
        .other_options(vec!["--locked".into()])
        .exec()
        .map_err(|error| format!("cargo metadata failed: {error}"))?;
    let mut packages = Vec::new();
    for package in metadata.workspace_packages() {
        for dependency in &package.dependencies {
            if let Some(path) = &dependency.path {
                let registered = metadata.workspace_packages().iter().any(|candidate| {
                    candidate.name == dependency.name
                        && candidate.manifest_path.parent() == Some(path.as_path())
                });
                if !registered {
                    return Err(format!(
                        "{}: local dependency {} at {} must be a registered workspace member",
                        package.name, dependency.name, path
                    ));
                }
            }
        }
        let path = package
            .manifest_path
            .parent()
            .ok_or("package has no parent directory")?
            .strip_prefix(&metadata.workspace_root)
            .map_err(|error| error.to_string())?
            .as_str()
            .replace('\\', "/");
        let layer = package
            .metadata
            .get("nai")
            .and_then(|nai| nai.get("layer"))
            .and_then(|layer| layer.as_str())
            .ok_or_else(|| format!("{} is missing package.metadata.nai.layer", package.name))?;
        packages.push(Package {
            name: package.name.to_string(),
            path,
            layer: layer.into(),
            dependencies: package
                .dependencies
                .iter()
                .map(|dep| dep.name.to_string())
                .collect(),
        });
    }
    let errors = validate(&packages);
    if errors.is_empty() {
        println!(
            "dependency boundaries passed: {} workspace packages (normal/build/dev/target declarations)",
            packages.len()
        );
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root =
                std::env::temp_dir().join(format!("nai-deps-{}-{nonce}", std::process::id()));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }

        fn write(&self, path: &str, content: &str) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }

        fn add_package(&self, name: &str, path: &str, layer: &str, dependencies: &str) {
            self.write(&format!("{path}/Cargo.toml"), &format!(
                "[package]\nname = {name:?}\nversion = \"0.1.0\"\nedition = \"2024\"\n[package.metadata.nai]\nlayer = {layer:?}\n{dependencies}\n"
            ));
            self.write(&format!("{path}/src/lib.rs"), "");
        }

        fn lock(&self) {
            assert!(
                Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
                    .args(["generate-lockfile", "--offline"])
                    .current_dir(&self.0)
                    .status()
                    .unwrap()
                    .success()
            );
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn real_metadata_catches_aliases_all_dependency_kinds_and_inactive_targets() {
        for section in [
            "dependencies",
            "build-dependencies",
            "dev-dependencies",
            "target.'cfg(target_os = \"none\")'.dependencies",
        ] {
            let fixture = Fixture::new();
            fixture.write("Cargo.toml", "[workspace]\nresolver = \"3\"\nmembers = [\"crates/nai-kernel\", \"plugins/platform-manual\"]\n");
            fixture.add_package("platform-manual", "plugins/platform-manual", "plugin", "");
            fixture.add_package("nai-kernel", "crates/nai-kernel", "kernel", &format!(
                "[{section}]\nharmless_alias = {{ package = \"platform-manual\", path = \"../../plugins/platform-manual\" }}"
            ));
            fixture.lock();
            let error = check(&fixture.0).unwrap_err();
            assert!(
                error.contains("nai-kernel (kernel) must not depend on platform-manual"),
                "{section}: {error}"
            );
        }
    }

    #[test]
    fn rejects_local_dependency_excluded_from_workspace() {
        let fixture = Fixture::new();
        fixture.write("Cargo.toml", "[workspace]\nresolver = \"3\"\nmembers = [\"crates/nai-kernel\"]\nexclude = [\"plugins/platform-manual\"]\n");
        fixture.add_package("platform-manual", "plugins/platform-manual", "plugin", "");
        fixture.add_package(
            "nai-kernel",
            "crates/nai-kernel",
            "kernel",
            "[dependencies]\nplatform-manual = { path = \"../../plugins/platform-manual\" }",
        );
        fixture.lock();
        assert!(
            check(&fixture.0)
                .unwrap_err()
                .contains("must be a registered workspace member")
        );
    }

    fn package(name: &str, path: &str, layer: &str, dependencies: &[&str]) -> Package {
        Package {
            name: name.into(),
            path: path.into(),
            layer: layer.into(),
            dependencies: dependencies.iter().map(|name| (*name).into()).collect(),
        }
    }

    #[test]
    fn keeper_and_worker_cannot_disguise_media_layer_or_import_provider() {
        for path in ["apps/output-keeper", "apps/render-worker"] {
            let provider = package(
                "nai-provider-core",
                "crates/nai-provider-core",
                "provider",
                &[],
            );
            let process = package("media-process", path, "media", &["nai-provider-core"]);
            assert!(!validate(&[process, provider]).is_empty());
            let disguised = package("media-process", path, "app", &[]);
            assert!(!validate(&[disguised]).is_empty());
        }
    }

    #[test]
    fn valid_kernel_sdk_and_composition_root() {
        let packages = vec![
            package("nai-contracts", "crates/nai-contracts", "contracts", &[]),
            package(
                "nai-kernel",
                "crates/nai-kernel",
                "kernel",
                &["nai-contracts", "tokio"],
            ),
            package(
                "nai-plugin-sdk",
                "crates/nai-plugin-sdk",
                "sdk",
                &["nai-contracts"],
            ),
            package(
                "platform-manual",
                "plugins/platform-manual",
                "plugin",
                &["nai-plugin-sdk"],
            ),
            package(
                "nai-runtime",
                "crates/nai-runtime",
                "runtime",
                &["nai-kernel", "platform-manual"],
            ),
        ];
        assert!(validate(&packages).is_empty());
    }

    #[test]
    fn rejects_contract_business_dependency_and_kernel_plugin() {
        let packages = vec![
            package(
                "nai-contracts",
                "crates/nai-contracts",
                "contracts",
                &["platform-manual"],
            ),
            package(
                "nai-kernel",
                "crates/nai-kernel",
                "kernel",
                &["platform-manual"],
            ),
            package("platform-manual", "plugins/platform-manual", "plugin", &[]),
        ];
        let errors = validate(&packages);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("nai-contracts (contracts) must not depend"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("nai-kernel (kernel) must not depend"))
        );
    }

    #[test]
    fn rejects_plugin_to_plugin_through_shared_library() {
        let packages = vec![
            package(
                "agent",
                "plugins/agent-default",
                "plugin",
                &["nai-knowledge-core"],
            ),
            package(
                "nai-knowledge-core",
                "crates/nai-knowledge-core",
                "shared",
                &["knowledge"],
            ),
            package("knowledge", "plugins/knowledge-local", "plugin", &[]),
        ];
        assert!(
            validate(&packages)
                .iter()
                .any(|error| error.contains("agent (plugin) must not depend on knowledge"))
        );
    }

    #[test]
    fn rejects_layer_disguise_and_unknown_paths() {
        for pkg in [
            package("plugin", "plugins/agent-default", "shared", &[]),
            package("new", "crates/unknown", "shared", &[]),
        ] {
            assert!(
                validate(&[pkg])
                    .iter()
                    .any(|error| error.contains("does not match"))
            );
        }
    }

    #[test]
    fn rejects_native_kernel_dependencies_and_media_provider_paths() {
        let packages = vec![
            package(
                "nai-kernel",
                "crates/nai-kernel",
                "kernel",
                &["windows-sys", "tauri"],
            ),
            package(
                "media",
                "apps/media-worker",
                "media",
                &["nai-provider-core"],
            ),
            package(
                "nai-provider-core",
                "crates/nai-provider-core",
                "provider",
                &["reqwest"],
            ),
        ];
        let errors = validate(&packages);
        assert!(errors.iter().any(|error| error.contains("windows-sys")));
        assert!(errors.iter().any(|error| error.contains("tauri")));
        assert!(
            errors
                .iter()
                .any(|error| error.contains("media (media) must not depend"))
        );
        assert!(errors.iter().any(|error| error.contains("reqwest")));
    }
}
