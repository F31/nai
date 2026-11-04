//! The workspace harness as a library.
//!
//! The check logic lives here, the CLI shell lives in `main.rs`. The split is
//! not cosmetic: `cargo xtask check --profile portable` runs workspace builds,
//! and on Windows the image that is currently executing cannot be relinked. With
//! the checks in a library the profile can test the harness without selecting —
//! and therefore without linking — the `nai-xtask` binary target.
pub mod deps;
pub mod frontend;
pub mod golden;
pub mod registry;
pub mod sbom;
pub mod schema;
pub mod secrets;

use std::{env, path::Path, process::Command};

/// The running package. Referenced instead of a literal so a rename cannot
/// leave the profile excluding a package that no longer exists.
pub const HARNESS: &str = env!("CARGO_PKG_NAME");

fn run(root: &Path, args: &[&str]) -> Result<(), String> {
    println!("cargo {}", args.join(" "));
    let status = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|error| format!("cannot run cargo: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed: {status}", args.join(" ")))
    }
}

/// V4.8 RF01–04 engineering scope; does not grant any complete ACC acceptance.
fn workbench_checks(root: &Path) -> Result<(), String> {
    deps::check(root)?;
    registry::check(root)?;
    run(
        root,
        &[
            "test",
            "-p",
            HARNESS,
            "--lib",
            "workbench_tests",
            "--locked",
        ],
    )?;
    schema::node(
        root,
        &[
            "--test",
            "tests/contracts/workbench-source.test.mjs",
            "tests/contracts/foundation.test.mjs",
            "tests/contracts/preparation.test.mjs",
            "tests/contracts/publication.test.mjs",
            "tests/contracts/workbench-ci.test.mjs",
            "tests/ui/v43-bootstrap.test.mjs",
            "tests/ui/v43-preparation.test.mjs",
            "tests/ui/v43-faq.test.mjs",
            "tests/ui/v43-llm.test.mjs",
        ],
    )?;
    run(
        root,
        &[
            "test",
            "-p",
            "nai-foundation-sqlite",
            "-p",
            "nai-runtime-core",
            "-p",
            "nai-workbench-engine",
            "-p",
            "nai-audition-worker",
            "-p",
            "nai-public-output-keeper",
            "-p",
            "nai-llm-openai",
            "--locked",
        ],
    )?;
    run(
        root,
        &["test", "-p", HARNESS, "--lib", "deps::tests", "--locked"],
    )?;
    run(
        root,
        &[
            "clippy",
            "-p",
            "nai-foundation-sqlite",
            "-p",
            "nai-runtime-core",
            "-p",
            "nai-workbench-engine",
            "-p",
            "nai-workbench-host",
            "-p",
            "nai-public-output-keeper",
            "-p",
            "nai-llm-openai",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    schema::node(root, &["scripts/workbench-frontend.mjs"])?;
    println!(
        "RF01–04 engineering scope passed; all 164 ACC remain pending, no complete product certification"
    );
    Ok(())
}

/// 两种工程 profile 已退役；原编排见tests/legacy/v311/engineering-profile.rs.txt。
pub fn execute(args: &[String], root: &Path) -> Result<(), String> {
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["check", "--profile", "workbench"] => workbench_checks(root),
        ["check-deps"] => deps::check(root),
        ["check", "--registry"] => registry::check(root),
        ["check", "--golden"] => golden::check(root, None),
        ["check", "--golden", "--dir", dir] => golden::check(root, Some(dir)),
        ["check", "--secrets"] => secrets::check(root),
        ["sbom"] => sbom::generate(root, None),
        ["sbom", "--out", path] => sbom::generate(root, Some(Path::new(path))),
        ["schema"] => schema::generate(root, false),
        ["schema", "--check"] => schema::generate(root, true),
        ["test", "--suite", "contracts"] => schema::contracts(root),
        ["test", "--suite", "knowledge"] => run(
            root,
            &[
                "test",
                "--locked",
                "--package",
                "nai-knowledge-core",
                "--package",
                "nai-storage",
            ],
        ),
        ["test", "--suite", "provider"] => run(
            root,
            &["test", "--locked", "--package", "nai-provider-core"],
        ),
        ["test", "--suite", "scheduler"] => run(
            root,
            &[
                "test",
                "--locked",
                "--package",
                "nai-kernel",
                "--package",
                "nai-budget-core",
                "--package",
                "nai-storage",
            ],
        ),
        ["test", "--suite", "session"] => run(
            root,
            &[
                "test",
                "--locked",
                "--package",
                "nai-kernel",
                "--package",
                "nai-media-core",
                "--package",
                "nai-storage",
            ],
        ),
        ["test", "--suite", "kernel"] => run(
            root,
            &[
                "test",
                "--locked",
                "--package",
                "nai-kernel",
                "--package",
                "nai-plugin-sdk",
            ],
        ),
        ["test", "--suite", "hosts"] => run(
            root,
            &[
                "test",
                "--locked",
                "--package",
                "nai-workbench-host",
                "--package",
                "nai-plugin-sdk",
            ],
        ),
        ["test", "--suite", "storage"] => run(
            root,
            &[
                "test",
                "--locked",
                "--package",
                "nai-storage",
                "--package",
                "nai-security",
            ],
        ),
        ["test", "--suite", "upgrade"] => run(
            root,
            &[
                "test",
                "--locked",
                "--package",
                "nai-upgrade-core",
                "--package",
                "nai-storage",
            ],
        ),
        ["check", "--profile", "portable" | "windows-local" | "full"] => Err(
            "LEGACY_PROFILE_RETIRED: use check --profile workbench; V4.8 release certification is not implemented".into()
        ),
        ["help"] | ["--help"] | [] => {
            println!(
                "Usage: cargo xtask check-deps | check --registry | check --golden [--dir <path>] | check --secrets | check --profile workbench | sbom [--out <path>] | schema [--check] | test --suite contracts|storage|kernel|hosts|session|scheduler|provider|knowledge|upgrade\nOther suites, certify and package tasks are not available yet."
            );
            Ok(())
        }
        _ => Err(format!(
            "unsupported or not implemented task: {} (use cargo xtask help)",
            args.join(" ")
        )),
    }
}

#[cfg(test)]
mod workbench_tests {
    #[test]
    fn retired_profiles_cannot_report_v43_success() {
        for profile in ["portable", "windows-local", "full"] {
            let args = ["check", "--profile", profile].map(String::from);
            assert!(
                super::execute(&args, std::path::Path::new("."))
                    .unwrap_err()
                    .starts_with("LEGACY_PROFILE_RETIRED")
            );
        }
    }
}
