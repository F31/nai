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
    // 静态契约 schema 校验（JSON Schema + OpenAPI）不需要 protoc，因此可以放在这里：
    // 在此之前它**只在 CI 里跑**，于是「改了 schema 的一侧却没同步样本」可以本机全绿、
    // CI 变红——W51 的 backup schema 版本漂移正是这样发生的。需要编译产物的校验
    // （`tests/contracts/descriptor.test.mjs`）不在此列，仍留在 contracts suite。
    schema::node(root, &["--test", "tests/contracts/schema.test.mjs"])?;
    // 平台能力矩阵与源码平台分支互相约束：矩阵必须登记每一项能力在每一个平台上的状态，
    // 且**全仓**含平台条件编译的文件必须是登记路径的子集。后者是本步存在的理由——
    // 在此之前，新增一处 `cfg(target_os = ...)` 不会引起任何地方注意，平台假设只增不减。
    // 同样是纯静态（读 JSON、扫源码、读 SDK 枚举标签），不需要 protoc 或编译产物。
    schema::node(root, &["--test", "tests/platform/platform-matrix.test.mjs"])?;
    // CI 工作流的步集合与分类的闭世界（W95）：它住在 `tests/contracts/` 下因为判据跟着契约模块走，
    // 但不需要 protoc，因此与上面两步同级。要编译产物的那一步（`descriptor.test.mjs`）不在此列。
    schema::node(root, &["--test", "tests/contracts/workflow.test.mjs"])?;
    // `execute()` 的命令面闭世界与 usage 字符串的双向对账（W96）：它住在 `tests/contracts/`
    // 下因为判据跟着契约模块走，但不需要 protoc，因此与上面几步同级。★ 这一行有一个副作用：
    // 它自己也是一条命令的证据源头——`check-deps` 与 `check --registry` 的门控落点就在本函数体里。
    schema::node(root, &["--test", "tests/contracts/xtask-commands.test.mjs"])?;
    // 被测面的集合守卫（W97）：门控跑哪些包、每个 suite 跑哪些包、CI 调用哪些命令，三者由实参
    // 解析成集合后做包含关系核验——不再只用「某个 marker 逐字存在」充当覆盖证明。同样纯静态：
    // 读 workspace 与成员清单、解析本文件的测试实参、读 CI workflow 文本，不需要 protoc。
    // ★ 副作用：本函数是它的**第四个**受审对象——它自己就是「门控闭包」的边界，
    // 把这里的 `--workspace` 换成任何显式列表，该测试会立刻算出被牺牲的包。
    schema::node(root, &["--test", "tests/contracts/gate-coverage.test.mjs"])?;
    // npm 脚本的执行面守卫（W98）：每个脚本「谁调用它」由解析算出，孤儿必须具名付代价；
    // 文档里的命令不算调用，自引用不算消费者。同样纯静态：读三份 `package.json`、解析驱动表、
    // 扫引用、读 `pnpm-workspace.yaml`，不需要 protoc。
    schema::node(root, &["--test", "tests/contracts/npm-scripts.test.mjs"])?;
    // 站点图层声明（`tier`）的兑付守卫（W99）：每个登记站点声明的「跑它的最低一层」由**宿主解析出的
    // 区域**核验——`portable` 靠本函数的 `engineering_checks` 区域（**剥注释后**），`full` 靠前端门控
    // 驱动表的 STEP 脚本体，`ci` 靠某个具名 workflow step 的 `run` 文本；且 `full`/`ci` 的 marker
    // MUST NOT 落回本区域（声明不得少报本机门控真正跑到的东西）。同样纯静态：解析本文件、驱动表、
    // workflow 与三份 `package.json`，不需要 protoc。★ 副作用：本函数体正是 portable 面的**判定域**，
    // 把上面任一行搬出本函数就会让对应站点立刻失去兑付。
    schema::node(root, &["--test", "tests/contracts/tier-payment.test.mjs"])?;
    // 在册判定步的**执行配置面**（`matrix` × `if:` × `continue-on-error`）的兑付守卫（W100）：
    // 每个步的**执行平台集合**由 job 的矩阵与步的条件**静态求值**得出，MUST 非空（一条「永不跑」的
    // 判定就是一条静默消失的判定）；产生判定的步 MUST NOT 带 `continue-on-error`（判定不可被吞），
    // 诊断步 MUST 带（诊断不得替换原始结论）；`ci-only` 的成因与平台条件 MUST 一致（双向）；
    // 矩阵形状、条件形状、`continue-on-error` 形状、runner 标签与 `runner.os` 取值 MUST 落在各自的
    // 闭集里；平台专属命令 MUST 被平台条件兑付。同样纯静态：解析 workflow 文本，不需要 protoc。
    schema::node(root, &["--test", "tests/contracts/step-config.test.mjs"])?;
    // 在册判定步的**目标绑定**（W101）：这一步跑的到底是哪个目标。`identity` 是**名字**，
    // 不是目标——把两个 suite 步的目标互换、或把一个判定步整条换成另一条合法判定命令，
    // 在 W95/W97/W100 三层上**全绿零信号**（W101 探针 W1–W4）。本步要求每个判定步的在册条目
    // 声明 `target`，并把它与从该步 `run` 文本**解析**出的目标集合**双向对账**；`infra`/`diagnostic`
    // MUST NOT 带目标（也不许解析出目标——把判定塞进「环境准备」是整条检查的后门）；
    // 目标的**形状**落在闭集里，域外取值具名失败。同样纯静态：解析本文件与 workflow 文本，
    // 不需要 protoc。★ 副作用：这一行是本步的受审对象之一——把它删掉，站点就失去兑付。
    schema::node(root, &["--test", "tests/contracts/step-target.test.mjs"])?;
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
        // 构建与测试。两者分开是因为**前置不同**：portable 需要 Rust 工具链、node 与**仓库根**的
        // 依赖（`check --registry` 与静态契约校验都是 node 脚本），前端步骤还需要各前端包自己的
        // 依赖与打包工具。把两者合并会让「前端依赖没装好」冒充成工程基础破了。前端步骤的失败与
        // 前置缺失由 `frontend::check` 具名报出，MUST NOT 被跳过或降级。
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
