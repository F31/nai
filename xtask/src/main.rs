mod deps;
mod registry;
mod schema;

use std::{env, path::Path, process::Command};

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

fn execute(args: &[String], root: &Path) -> Result<(), String> {
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["check-deps"] => deps::check(root),
        ["check", "--registry"] => registry::check(root),
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
            run(root, &["test", "--workspace", "--locked"])?;
            println!(
                "{profile}: engineering foundation checks passed; device/SDK/GPU/install certification not performed"
            );
            Ok(())
        }
        ["help"] | ["--help"] | [] => {
            println!(
                "Usage: cargo xtask check-deps | check --registry | check --profile portable|windows-local | schema [--check] | test --suite contracts|storage|kernel|hosts|session|scheduler|provider|knowledge\nOther suites, certify and package tasks are not available yet."
            );
            Ok(())
        }
        _ => Err(format!(
            "unsupported or not implemented task: {} (use cargo xtask help)",
            args.join(" ")
        )),
    }
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is at workspace root");
    if let Err(error) = execute(&env::args().skip(1).collect::<Vec<_>>(), root) {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}
