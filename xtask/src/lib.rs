//! The workspace harness as a library.
//!
//! The check logic lives here, the CLI shell lives in `main.rs`. The split is
//! not cosmetic: `cargo xtask check --profile portable` runs workspace builds,
//! and on Windows the image that is currently executing cannot be relinked. With
//! the checks in a library the profile can test the harness without selecting —
//! and therefore without linking — the `nai-xtask` binary target.
pub mod deps;
pub mod golden;
pub mod registry;
pub mod schema;

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

pub fn execute(args: &[String], root: &Path) -> Result<(), String> {
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["check-deps"] => deps::check(root),
        ["check", "--registry"] => registry::check(root),
        ["check", "--golden"] => golden::check(root, None),
        ["check", "--golden", "--dir", dir] => golden::check(root, Some(dir)),
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
                "nai-plugin-hosts",
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
        [
            "check",
            "--profile",
            profile @ ("portable" | "windows-local"),
        ] => {
            if *profile == "windows-local" && !cfg!(target_os = "windows") {
                return Err("windows-local requires a native Windows environment".into());
            }
            deps::check(root)?;
            registry::check(root)?;
            run(root, &["fmt", "--all", "--check"])?;
            run(
                root,
                &[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--locked",
                    "--",
                    "-D",
                    "warnings",
                ],
            )?;
            // A running executable cannot be relinked where the OS locks it, and
            // this harness is a workspace member: a whole-workspace test run also
            // names the harness binary target, which would relink the image that
            // is executing right now. On Windows that is a hard linker failure
            // (LNK1104) that says nothing about the workspace. Run every other
            // package here, then cover the harness itself through its library,
            // which never links the binary. The clippy step above already checks
            // the harness's binary target, so nothing is left unverified.
            run(
                root,
                &["test", "--workspace", "--locked", "--exclude", HARNESS],
            )?;
            run(root, &["test", "--locked", "--package", HARNESS, "--lib"])?;
            println!(
                "{profile}: engineering foundation checks passed; device/SDK/GPU/install certification not performed"
            );
            Ok(())
        }
        ["help"] | ["--help"] | [] => {
            println!(
                "Usage: cargo xtask check-deps | check --registry | check --golden [--dir <path>] | check --profile portable|windows-local | schema [--check] | test --suite contracts|storage|kernel|hosts|session|scheduler|provider|knowledge|upgrade\nOther suites, certify and package tasks are not available yet."
            );
            Ok(())
        }
        _ => Err(format!(
            "unsupported or not implemented task: {} (use cargo xtask help)",
            args.join(" ")
        )),
    }
}
