//! Shared isolated avatar process environment. No credentials or proxy inherited.
use super::v561_avatar_worker::Config;
use nai_contracts::workbench::WorkbenchError as Error;
use std::{
    path::Path,
    process::{Command, Stdio},
};
pub(crate) fn command(
    config: &Config,
    script: &Path,
    config_path: &Path,
    token: &[u8],
    log: &Path,
) -> Result<Command, Error> {
    let home = config.temp_dir.join("home");
    let matplotlib = config.temp_dir.join("matplotlib");
    for directory in [&home, &matplotlib] {
        std::fs::create_dir_all(directory).map_err(|_| Error::IoFailed)?;
    }
    let mut command = Command::new(&config.python);
    command.env_clear();
    // Explicit native compiler environment for the Windows maintenance
    // profile. Credentials/proxy/user variables remain absent.
    for name in [
        "SystemRoot",
        "WINDIR",
        "PATH",
        "INCLUDE",
        "LIB",
        "LIBPATH",
        "CUDA_PATH",
        "CUDA_HOME",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let paths = std::env::join_paths(
        config
            .dependency_dirs
            .iter()
            .chain(std::iter::once(&config.upstream_dir)),
    )
    .map_err(|_| Error::InvalidArgument)?;
    command
        .env("PYTHONPATH", paths)
        .env("PYTHONUTF8", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("PYTHONFAULTHANDLER", "1")
        .env("USE_TF", "0")
        .env("HF_HUB_OFFLINE", "1")
        .env("TRANSFORMERS_OFFLINE", "1")
        .env("TEMP", &config.temp_dir)
        .env("TMP", &config.temp_dir)
        .env("USERPROFILE", &home)
        .env("HOME", &home)
        .env("MPLCONFIGDIR", &matplotlib)
        .env("MPLBACKEND", "Agg")
        .env("HF_HOME", config.temp_dir.join("huggingface"))
        .env("TORCH_HOME", config.temp_dir.join("torch"))
        .env("TRITON_CACHE_DIR", config.temp_dir.join("triton"))
        .env("CUDA_CACHE_PATH", config.temp_dir.join("cuda"))
        .env("TORCHINDUCTOR_CACHE_DIR", config.temp_dir.join("inductor"))
        .env("TORCH_EXTENSIONS_DIR", config.temp_dir.join("extensions"))
        .env(
            "NAI_V561_AVATAR_TOKEN",
            token.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        )
        .arg(script)
        .arg("--config")
        .arg(config_path)
        .current_dir(&config.upstream_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(std::fs::File::create_new(log).map_err(|_| Error::IoFailed)?);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    Ok(command)
}
