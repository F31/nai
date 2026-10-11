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

/// 两种工程 profile 共用的基础检查。
///
/// `portable` 与 `full` 都从这里开始，因此「`full` 包含 `portable` 的全部步骤」是**代码结构
/// 事实**，而不是一份需要手工同步、可能漂移的清单。
fn engineering_checks(root: &Path) -> Result<(), String> {
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
    Ok(())
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
            engineering_checks(root)?;
            println!(
                "{profile}: engineering foundation checks passed; device/SDK/GPU/install certification not performed"
            );
            Ok(())
        }
        // `full` 是「完整工程基础」：portable 的全部步骤，加上前端 workspace 的类型检查、
        // 构建与测试。`portable` 刻意不带前端——它的名字承诺「只需 Rust 工具链就能跑」，
        // 而前端步骤需要已安装的 node 依赖；把两者合并会让「刚 clone、尚未 install」的环境
        // 变红，而那既不是 Rust 破了也不是前端破了。前端步骤的失败与前置缺失由
        // `frontend::check` 具名报出，MUST NOT 被跳过或降级。
        ["check", "--profile", "full"] => {
            engineering_checks(root)?;
            frontend::check(root)?;
            println!(
                "full: engineering foundation checks incl. the frontend gate passed; device/SDK/GPU/install certification not performed"
            );
            Ok(())
        }
        ["help"] | ["--help"] | [] => {
            println!(
                "Usage: cargo xtask check-deps | check --registry | check --golden [--dir <path>] | check --secrets | check --profile portable|windows-local|full | sbom [--out <path>] | schema [--check] | test --suite contracts|storage|kernel|hosts|session|scheduler|provider|knowledge|upgrade\nOther suites, certify and package tasks are not available yet."
            );
            Ok(())
        }
        _ => Err(format!(
            "unsupported or not implemented task: {} (use cargo xtask help)",
            args.join(" ")
        )),
    }
}
