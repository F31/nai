//! 前端 workspace 的工程门控入口。
//!
//! 这里只决定「何时跑、以什么身份跑」；「跑什么」在 `scripts/frontend-gate.mjs`——它读各包
//! `package.json` 的 `scripts`，那才是前端检查命令的**单一真源**。拆成两层有具体原因：前端
//! 检查天然是 JS 生态的问题（读 `package.json`、解析 npm 包入口、展开 glob），而本仓库的
//! `node_modules` 由 Linux 容器安装，`node_modules/.bin/*` 是一千字节左右的**无扩展名 POSIX
//! 垫片**，Windows 上既不能直接执行也不是可识别命令。若让门控去 shell 调用 `pnpm`，那么
//! 「pnpm 在不在 PATH 上」「垫片与本机 ABI 是否匹配」这些**主机环境事实**就会冒充前端结论。
//!
//! 因此：本模块只负责发起子进程并把失败翻译成**具名诊断**——缺工具链、脚本失败都在这里
//! 成为可读的错误，绝不静默跳过某一步再报告门控通过。

use std::{env, ffi::OsStr, path::Path, process::Command};

/// 门控驱动的脚本（相对仓库根）。
pub const SCRIPT: &str = "scripts/frontend-gate.mjs";

/// 找不到 `node` 时的具名标记。读者据此可以区分「前端破了」与「这台机器没有前端工具链」。
pub const TOOLCHAIN_MISSING: &str = "FRONTEND_TOOLCHAIN_MISSING";

/// 运行前端门控。前端步骤的失败与前置缺失都据此以具名诊断返回。
pub fn check(root: &Path) -> Result<(), String> {
    let program = env::var_os("NODE").unwrap_or_else(|| "node".into());
    run(&program, root)
}

fn run(program: &OsStr, root: &Path) -> Result<(), String> {
    println!("node {SCRIPT}");
    let status = Command::new(program)
        .arg(SCRIPT)
        .current_dir(root)
        .status()
        .map_err(|error| {
            format!(
                "{TOOLCHAIN_MISSING}: 无法运行 `{}`（{error}）；前端门控需要 node，可用 NODE 环境变量指定解释器",
                program.to_string_lossy()
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("前端门控失败（node {SCRIPT}）：{status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_toolchain_is_named_rather_than_skipped() {
        let error = run(OsStr::new("nai-definitely-not-a-program"), Path::new("."))
            .expect_err("不存在的解释器必须失败，而不是被跳过");
        assert!(error.contains(TOOLCHAIN_MISSING), "{error}");
        assert!(error.contains("nai-definitely-not-a-program"), "{error}");
    }
}
