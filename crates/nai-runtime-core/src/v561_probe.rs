//! V5.6.1 local dependency probe. No model inference, no network beyond a
//! loopback HTTP reachability check, no credential reads.

use serde::Serialize;
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Warn,
    Missing,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Overall {
    Ok,
    Degraded,
    Blocked,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProbeCheck {
    pub id: &'static str,
    pub label: &'static str,
    pub status: CheckStatus,
    pub required: bool,
    pub detail: String,
    pub fix: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProbeReport {
    pub baseline: &'static str,
    pub timestamp_ms: u128,
    pub overall: Overall,
    pub checks: Vec<ProbeCheck>,
}

#[derive(Debug)]
struct CommandResult {
    success: bool,
    stdout: String,
    stderr: String,
}

pub fn probe(root: &Path, config: &Value) -> ProbeReport {
    let checks = vec![
        check_data_root(root, config),
        check_config(root, config),
        check_qwen(config),
        check_flashhead(root, config),
        check_python(config),
        check_torch(config),
        check_nvidia(),
        check_ffmpeg(),
        check_obs(config),
        check_audio(config),
    ];

    let overall = rollup(&checks);
    ProbeReport {
        baseline: "V5.6.1",
        timestamp_ms: now_ms(),
        overall,
        checks,
    }
}

fn check_data_root(root: &Path, _config: &Value) -> ProbeCheck {
    let required = ["assets", "logs", "recordings"];
    let missing: Vec<_> = required
        .iter()
        .filter(|dir| !root.join(dir).is_dir())
        .copied()
        .collect();
    if missing.is_empty() {
        ok(
            "data-root",
            "数据根",
            true,
            format!("{}", root.display()),
            "检查目录权限",
        )
    } else {
        missing_check(
            "data-root",
            "数据根",
            true,
            format!("缺少目录: {}", missing.join(", ")),
            "运行 nai-v561-probe 的 open 流程创建目录，或手动创建 assets/logs/recordings",
        )
    }
}

fn check_config(root: &Path, config: &Value) -> ProbeCheck {
    let path = root.join("config.json");
    if !path.is_file() {
        return missing_check(
            "config",
            "配置文件",
            true,
            format!("缺少 {}", path.display()),
            "保留数据根和现有资料，恢复 config.json 或选择有效配置",
        );
    }
    if !config.is_object() {
        return missing_check(
            "config",
            "配置文件",
            true,
            "config 根不是对象".to_string(),
            "修正 config.json 为 JSON 对象",
        );
    }
    let engine = config
        .pointer("/avatar/engine")
        .and_then(Value::as_str)
        .unwrap_or("");
    if engine != "flashhead" {
        return warn(
            "config",
            "配置文件",
            true,
            format!("avatar.engine={engine:?}，期望 flashhead"),
            "把 avatar.engine 改为 flashhead；V5.6.1 不加载双引擎",
        );
    }
    ok(
        "config",
        "配置文件",
        true,
        format!("{} 可读", path.display()),
        "修改后重新运行探测",
    )
}

fn check_qwen(config: &Value) -> ProbeCheck {
    let endpoint = config
        .pointer("/tts/endpoint")
        .and_then(Value::as_str)
        .unwrap_or("http://127.0.0.1:8000/v1");
    let health = qwen_health_url(endpoint);
    match http_status(&health, Duration::from_secs(2)) {
        Ok(status) if (200..300).contains(&status) => warn(
            "qwen",
            "Qwen3-TTS",
            true,
            format!(
                "{health} -> HTTP {status}；仅证明服务可达，尚未验证模型或语音能力；endpoint {endpoint}"
            ),
            "核对模型和音色，运行语音能力探测",
        ),
        Ok(status) => warn(
            "qwen",
            "Qwen3-TTS",
            true,
            format!("{health} 可达但返回 HTTP {status}; endpoint {endpoint}"),
            "确认 endpoint 是 Qwen3-TTS 健康/生成接口，或修正 tts.endpoint",
        ),
        Err(error) => missing_check(
            "qwen",
            "Qwen3-TTS",
            true,
            format!("{endpoint} 不可达: {error}"),
            "启动本地 Qwen3-TTS，确认端口与 tts.endpoint；未就绪时只能文字模式",
        ),
    }
}

fn check_flashhead(root: &Path, config: &Value) -> ProbeCheck {
    let configured = config
        .pointer("/avatar/model_dir")
        .and_then(Value::as_str)
        .unwrap_or("assets/models/flashhead");
    let dir = if Path::new(configured).is_absolute() {
        PathBuf::from(configured)
    } else {
        root.join(configured)
    };
    if !dir.is_dir() {
        return missing_check(
            "flashhead",
            "FlashHead 权重",
            true,
            format!("缺少 {}", dir.display()),
            "下载合法的 FlashHead Lite 权重到 avatar.model_dir；同时核查权重/VAE 许可",
        );
    }
    match std::fs::read_dir(&dir) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                missing_check(
                    "flashhead",
                    "FlashHead 权重",
                    true,
                    format!("{} 非空，但权重清单、哈希与可加载性尚未验证", dir.display()),
                    "确认官方权重及许可，完成完整性校验和模型加载验证",
                )
            } else {
                warn(
                    "flashhead",
                    "FlashHead 权重",
                    true,
                    format!("{} 为空", dir.display()),
                    "放入合法权重；空目录不能视为已安装",
                )
            }
        }
        Err(error) => missing_check(
            "flashhead",
            "FlashHead 权重",
            true,
            format!("读取 {} 失败: {error}", dir.display()),
            "检查路径权限与目录是否可读",
        ),
    }
}

fn check_python(config: &Value) -> ProbeCheck {
    let python = config
        .pointer("/avatar/worker_python")
        .and_then(Value::as_str)
        .unwrap_or("python");
    match run_capture(python, &["--version"], Duration::from_secs(3)) {
        Ok(result) if result.success => ok(
            "python",
            "Python worker",
            true,
            first_line(&result.stdout, &result.stderr),
            "无需处理",
        ),
        Ok(result) => missing_check(
            "python",
            "Python worker",
            true,
            format!(
                "{python} 退出失败: {}",
                first_line(&result.stdout, &result.stderr)
            ),
            "安装 Python 3.11 或配置 avatar.worker_python 指向 bundled venv",
        ),
        Err(error) => missing_check(
            "python",
            "Python worker",
            true,
            format!("{python} 不可执行: {error}"),
            "安装 Python 3.11 或配置 avatar.worker_python",
        ),
    }
}

fn check_torch(config: &Value) -> ProbeCheck {
    let python = config
        .pointer("/avatar/worker_python")
        .and_then(Value::as_str)
        .unwrap_or("python");
    let code = "import torch; print(torch.__version__); print(torch.cuda.is_available())";
    match run_capture(python, &["-c", code], Duration::from_secs(20)) {
        Ok(result) if result.success => {
            let lines: Vec<_> = result.stdout.lines().map(str::trim).collect();
            let version = lines.first().copied().unwrap_or("unknown");
            let cuda = lines.get(1).copied().unwrap_or("unknown");
            if cuda.eq_ignore_ascii_case("true") {
                ok(
                    "torch",
                    "PyTorch/CUDA",
                    true,
                    format!("torch {version}; CUDA true"),
                    "运行 M0 记录显存/温度",
                )
            } else {
                warn(
                    "torch",
                    "PyTorch/CUDA",
                    true,
                    format!("torch {version}; CUDA {cuda}"),
                    "确认 NVIDIA 驱动/CUDA wheel；CPU 模式不能作为目标机 M0 通过",
                )
            }
        }
        Ok(result) => missing_check(
            "torch",
            "PyTorch/CUDA",
            true,
            first_line(&result.stdout, &result.stderr),
            "在 avatar worker venv 安装匹配 CUDA 的 PyTorch；不要用系统 Python 猜测",
        ),
        Err(error) => missing_check(
            "torch",
            "PyTorch/CUDA",
            true,
            format!("torch 探测失败: {error}"),
            "检查 worker venv 与超时；导入 torch 不应长期阻塞",
        ),
    }
}

fn check_nvidia() -> ProbeCheck {
    match run_capture(
        "nvidia-smi",
        &[
            "--query-gpu=name,driver_version,memory.total",
            "--format=csv,noheader",
        ],
        Duration::from_secs(4),
    ) {
        Ok(result) if result.success && !result.stdout.trim().is_empty() => ok(
            "gpu",
            "NVIDIA GPU",
            true,
            result
                .stdout
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string(),
            "运行 M0 记录 VRAM/温度/功耗/FPS",
        ),
        Ok(result) => missing_check(
            "gpu",
            "NVIDIA GPU",
            true,
            first_line(&result.stdout, &result.stderr),
            "安装 NVIDIA 驱动并确认 nvidia-smi 在 PATH",
        ),
        Err(error) => missing_check(
            "gpu",
            "NVIDIA GPU",
            true,
            format!("nvidia-smi 不可执行: {error}"),
            "安装 NVIDIA 驱动；目标机为 RTX 5090 Laptop 24GB",
        ),
    }
}

fn check_ffmpeg() -> ProbeCheck {
    match run_capture("ffmpeg", &["-version"], Duration::from_secs(3)) {
        Ok(result) if result.success => ok(
            "ffmpeg",
            "ffmpeg",
            true,
            result
                .stdout
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string(),
            "无需处理",
        ),
        Ok(result) => missing_check(
            "ffmpeg",
            "ffmpeg",
            true,
            first_line(&result.stdout, &result.stderr),
            "安装 ffmpeg 并加入 PATH；记录构建许可",
        ),
        Err(error) => missing_check(
            "ffmpeg",
            "ffmpeg",
            true,
            format!("ffmpeg 不可执行: {error}"),
            "安装 ffmpeg 并加入 PATH",
        ),
    }
}

fn check_obs(config: &Value) -> ProbeCheck {
    let configured = config.pointer("/output/obs_path").and_then(Value::as_str);
    match configured {
        Some(path) if Path::new(path).is_file() => ok(
            "obs",
            "OBS",
            false,
            path.to_string(),
            "启动 OBS 并确认窗口捕获/音轨/信号",
        ),
        Some(path) => warn(
            "obs",
            "OBS",
            false,
            format!("配置了 {path} 但文件不存在"),
            "修正 output.obs_path 或安装 OBS；仅本机预览不要求 OBS",
        ),
        None => warn(
            "obs",
            "OBS",
            false,
            "未配置 output.obs_path，OBS 状态未知".into(),
            "安装 OBS 并配置路径；进程存在不等于已捕获/已开播",
        ),
    }
}

fn check_audio(config: &Value) -> ProbeCheck {
    match config
        .pointer("/output/audio_device")
        .and_then(Value::as_str)
    {
        Some(device) if !device.is_empty() => unknown(
            "audio",
            "音频设备",
            false,
            format!("已配置 {device}；枚举/实际信号尚未接入"),
            "下一步接 CPAL 设备枚举与默认设备丢失检测",
        ),
        _ => unknown(
            "audio",
            "音频设备",
            false,
            "未配置输出设备；设备枚举尚未接入".into(),
            "在 1.3/1.5 接入 CPAL/WASAPI 枚举与设备丢失保护",
        ),
    }
}

fn qwen_health_url(endpoint: &str) -> String {
    let endpoint = endpoint.trim_end_matches('/');
    let root = endpoint.strip_suffix("/v1").unwrap_or(endpoint);
    format!("{root}/health")
}

fn rollup(checks: &[ProbeCheck]) -> Overall {
    if checks
        .iter()
        .any(|c| c.required && matches!(c.status, CheckStatus::Missing))
    {
        Overall::Blocked
    } else if checks.iter().any(|c| {
        matches!(
            c.status,
            CheckStatus::Warn | CheckStatus::Unknown | CheckStatus::Missing
        )
    }) {
        Overall::Degraded
    } else {
        Overall::Ok
    }
}

fn ok(
    id: &'static str,
    label: &'static str,
    required: bool,
    detail: String,
    fix: &str,
) -> ProbeCheck {
    ProbeCheck {
        id,
        label,
        status: CheckStatus::Ok,
        required,
        detail,
        fix: fix.into(),
    }
}

fn warn(
    id: &'static str,
    label: &'static str,
    required: bool,
    detail: String,
    fix: &str,
) -> ProbeCheck {
    ProbeCheck {
        id,
        label,
        status: CheckStatus::Warn,
        required,
        detail,
        fix: fix.into(),
    }
}

fn unknown(
    id: &'static str,
    label: &'static str,
    required: bool,
    detail: String,
    fix: &str,
) -> ProbeCheck {
    ProbeCheck {
        id,
        label,
        status: CheckStatus::Unknown,
        required,
        detail,
        fix: fix.into(),
    }
}

fn missing_check(
    id: &'static str,
    label: &'static str,
    required: bool,
    detail: String,
    fix: &str,
) -> ProbeCheck {
    ProbeCheck {
        id,
        label,
        status: CheckStatus::Missing,
        required,
        detail,
        fix: fix.into(),
    }
}

fn first_line(stdout: &str, stderr: &str) -> String {
    stdout
        .lines()
        .chain(stderr.lines())
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("无输出")
        .to_string()
}

fn run_capture(program: &str, args: &[&str], timeout: Duration) -> Result<CommandResult, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut pipe) = child.stdout.take() {
                    let _ = pipe.read_to_string(&mut stdout);
                }
                if let Some(mut pipe) = child.stderr.take() {
                    let _ = pipe.read_to_string(&mut stderr);
                }
                return Ok(CommandResult {
                    success: status.success(),
                    stdout,
                    stderr,
                });
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("timeout after {}ms", timeout.as_millis()));
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn http_status(url: &str, timeout: Duration) -> Result<u16, String> {
    let (host, port, path) = parse_http_url(url)?;
    let authority = format!("{host}:{port}");
    let address = authority
        .to_socket_addrs()
        .map_err(|error| error.to_string())?
        .next()
        .ok_or_else(|| "no address".to_string())?;
    let mut stream =
        TcpStream::connect_timeout(&address, timeout).map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|error| error.to_string())?;
    line.split_whitespace()
        .nth(1)
        .ok_or_else(|| format!("invalid HTTP status line: {line:?}"))?
        .parse::<u16>()
        .map_err(|error| error.to_string())
}

fn parse_http_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| "only http:// loopback is supported".to_string())?;
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{path}")),
        None => (rest, "/".to_string()),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (
            host.to_string(),
            port.parse::<u16>().map_err(|error| error.to_string())?,
        ),
        None => (authority.to_string(), 80),
    };
    if !matches!(host.as_str(), "127.0.0.1" | "localhost" | "[::1]") {
        return Err(format!("refusing non-loopback host {host}"));
    }
    if port == 0 {
        return Err("port must be non-zero".into());
    }
    Ok((host, port, path))
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_loopback_http_only() {
        assert_eq!(
            parse_http_url("http://127.0.0.1:8000/").unwrap(),
            ("127.0.0.1".into(), 8000, "/".into())
        );
        assert_eq!(
            parse_http_url("http://localhost:8780/api").unwrap(),
            ("localhost".into(), 8780, "/api".into())
        );
        assert!(parse_http_url("https://127.0.0.1:8000/").is_err());
        assert!(parse_http_url("http://example.com/").is_err());
        assert!(parse_http_url("http://127.0.0.1:0/").is_err());
    }

    #[test]
    fn qwen_health_url_derives_root_from_v1_or_root() {
        assert_eq!(
            qwen_health_url("http://127.0.0.1:8000/v1"),
            "http://127.0.0.1:8000/health"
        );
        assert_eq!(
            qwen_health_url("http://127.0.0.1:8000/"),
            "http://127.0.0.1:8000/health"
        );
        assert_eq!(
            qwen_health_url("http://127.0.0.1:8000/v1/"),
            "http://127.0.0.1:8000/health"
        );
    }

    #[test]
    fn rollup_blocks_required_missing_and_degrades_unknown() {
        let required_missing = ProbeCheck {
            id: "x",
            label: "x",
            status: CheckStatus::Missing,
            required: true,
            detail: String::new(),
            fix: String::new(),
        };
        assert_eq!(rollup(&[required_missing]), Overall::Blocked);
        let optional_unknown = ProbeCheck {
            id: "x",
            label: "x",
            status: CheckStatus::Unknown,
            required: false,
            detail: String::new(),
            fix: String::new(),
        };
        assert_eq!(rollup(&[optional_unknown]), Overall::Degraded);
        let good = ProbeCheck {
            id: "x",
            label: "x",
            status: CheckStatus::Ok,
            required: true,
            detail: String::new(),
            fix: String::new(),
        };
        assert_eq!(rollup(&[good]), Overall::Ok);
    }

    #[test]
    fn nonempty_model_directory_is_not_treated_as_ready() {
        let dir = tempfile::tempdir().unwrap();
        let model = dir.path().join("assets/models/flashhead");
        std::fs::create_dir_all(&model).unwrap();
        std::fs::write(model.join("unrelated.txt"), "not weights").unwrap();
        let check = check_flashhead(dir.path(), &json!({}));
        assert_eq!(check.status, CheckStatus::Missing);
        assert!(check.detail.contains("尚未验证"));
    }
    #[test]
    fn config_engine_mismatch_is_explicit() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["assets", "logs", "recordings"] {
            std::fs::create_dir_all(dir.path().join(name)).unwrap();
        }
        std::fs::write(dir.path().join("config.json"), "{}").unwrap();
        let check = check_config(
            dir.path(),
            &json!({"avatar": {"engine": "unsupported-engine"}}),
        );
        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check.detail.contains("unsupported-engine"));
    }
}
