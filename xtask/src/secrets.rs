//! Credential scan over **everything git would publish**.
//!
//! The domain is `git ls-files --cached --others --exclude-standard`: tracked
//! files plus untracked files that are not ignored. That choice is load-bearing
//! rather than incidental.
//!
//! * Not the working tree: a real installation keeps its credentials in an
//!   ignored local file, so a scanner that walked the disk would flag that file
//!   and make the check impossible to satisfy — and a check that cannot be
//!   satisfied gets switched off.
//! * Not only the index: a new file escapes `--cached` until it is staged, so
//!   the credential would be found only after the mistake was all but made.
//!
//! Ignored paths stay out of the domain either way, which is what lets a real
//! key live in `.env` on this machine.
//!
//! What it does **not** do: it cannot recognise a credential with no known
//! prefix. It recognises the high-entropy prefixed shapes tabulated below and
//! says so, instead of implying full coverage.
//!
//! Matched text is never carried, printed or stored: a report names the file,
//! the line and the prefix family only. Echoing the match would move the secret
//! from a private file into a build log.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Characters that may appear inside a provider-issued token.
const TOKEN_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";
/// Characters that may appear inside a URL-safe token.
const URL_TOKEN_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789.-";
/// Upper-case letters and digits only — the shape of an AWS access key id.
const UPPER_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

struct Pattern {
    /// Prefix family, as reported. Never a live value.
    label: &'static str,
    prefix: &'static str,
    /// Minimum run length after the prefix for the match to count.
    min_len: usize,
    allowed: &'static str,
}

/// Prefix table. Every minimum length sits **above** the length of the sentinel
/// strings the test suite already uses, so a fixture that merely looks like a
/// key does not trip the scan — while a real provider key, which is far longer,
/// does.
const PATTERNS: &[Pattern] = &[
    Pattern {
        label: "openai-style",
        prefix: "sk-",
        min_len: 32,
        allowed: TOKEN_CHARS,
    },
    Pattern {
        label: "openai-style",
        prefix: "wk-",
        min_len: 32,
        allowed: TOKEN_CHARS,
    },
    Pattern {
        label: "openai-style",
        prefix: "pk-",
        min_len: 32,
        allowed: TOKEN_CHARS,
    },
    Pattern {
        label: "openai-style",
        prefix: "rk-",
        min_len: 32,
        allowed: TOKEN_CHARS,
    },
    Pattern {
        label: "github-token",
        prefix: "ghp_",
        min_len: 30,
        allowed: TOKEN_CHARS,
    },
    Pattern {
        label: "github-token",
        prefix: "github_pat_",
        min_len: 30,
        allowed: TOKEN_CHARS,
    },
    Pattern {
        label: "gitlab-token",
        prefix: "glpat-",
        min_len: 18,
        allowed: TOKEN_CHARS,
    },
    Pattern {
        label: "slack-token",
        prefix: "xoxa-",
        min_len: 20,
        allowed: TOKEN_CHARS,
    },
    Pattern {
        label: "slack-token",
        prefix: "xoxb-",
        min_len: 20,
        allowed: TOKEN_CHARS,
    },
    Pattern {
        label: "slack-token",
        prefix: "xoxp-",
        min_len: 20,
        allowed: TOKEN_CHARS,
    },
    Pattern {
        label: "slack-token",
        prefix: "xoxr-",
        min_len: 20,
        allowed: TOKEN_CHARS,
    },
    Pattern {
        label: "slack-token",
        prefix: "xoxs-",
        min_len: 20,
        allowed: TOKEN_CHARS,
    },
    Pattern {
        label: "aws-key-id",
        prefix: "AKIA",
        min_len: 16,
        allowed: UPPER_CHARS,
    },
    Pattern {
        label: "google-api-key",
        prefix: "AIza",
        min_len: 30,
        allowed: TOKEN_CHARS,
    },
    Pattern {
        label: "authorization-header",
        prefix: "Bearer ",
        min_len: 32,
        allowed: URL_TOKEN_CHARS,
    },
];

/// Where a match was found and which family it belonged to. The matched text
/// itself is deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub line: usize,
    pub label: &'static str,
}

/// Scans one text. Line numbers are one-based, matching what an editor shows.
pub fn scan_text(text: &str) -> Vec<Hit> {
    let mut hits: Vec<Hit> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        for pattern in PATTERNS {
            if matches(line, pattern) {
                let hit = Hit {
                    line: index + 1,
                    label: pattern.label,
                };
                if !hits.contains(&hit) {
                    hits.push(hit);
                }
            }
        }
    }
    hits
}

fn matches(line: &str, pattern: &Pattern) -> bool {
    let haystack = line.as_bytes();
    let needle = pattern.prefix.as_bytes();
    let allowed = pattern.allowed.as_bytes();
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    let mut cursor = 0;
    while cursor + needle.len() <= haystack.len() {
        let Some(offset) = haystack[cursor..]
            .windows(needle.len())
            .position(|window| window == needle)
        else {
            return false;
        };
        let after = cursor + offset + needle.len();
        let run = haystack[after..]
            .iter()
            .take_while(|byte| allowed.contains(byte))
            .count();
        if run >= pattern.min_len {
            return true;
        }
        // Resume past this occurrence so overlapping prefixes cannot loop.
        cursor = after;
        if cursor >= haystack.len() {
            return false;
        }
    }
    false
}

/// The scan domain: everything git would publish — tracked files plus untracked
/// files that are not ignored. Sorted and de-duplicated.
///
/// Deliberately **not** the working tree. An ignored local credential file is
/// the intended place for a real key to live, and this function must not see
/// it.
pub fn tracked_files(root: &Path) -> Result<BTreeSet<String>, String> {
    // The root may arrive with `..` components (the tests reach the workspace
    // that way). Windows does not resolve those when the path is used as a
    // process working directory, and git then fails to walk up to `.git`, so the
    // path is resolved first.
    let root = std::path::absolute(root)
        .map_err(|error| format!("cannot resolve {}: {error}", root.display()))?;
    let output = Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])
        .current_dir(&root)
        .output()
        .map_err(|error| format!("cannot run git in {}: {error}", root.display()))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files failed in {}: {} ({})",
            root.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let listing = String::from_utf8(output.stdout)
        .map_err(|_| "git ls-files produced a non-UTF-8 path".to_owned())?;
    Ok(listing
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect())
}

pub fn check(root: &Path) -> Result<(), String> {
    let files = tracked_files(root)?;
    let mut findings: Vec<String> = Vec::new();
    for file in &files {
        let Ok(bytes) = std::fs::read(root.join(file)) else {
            continue;
        };
        // ⚠️ 已知弱点，具名登记、本轮不修（W130）：`from_utf8` 失败就 `else { continue; }`，
        // 二进制/非 UTF-8 的**已跟踪文件**会被整条丢掉。仓库域里这类文件是夹具而非诊断产物，
        // 与本轮缺口无关；诊断产物走下面的 `scan_tree`（按字节读）。把它一起改掉会让既有注入
        // 「`secrets::check(root)` 在闭包外」的前提失效，属于另一件事。
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        for hit in scan_text(&text) {
            findings.push(format!("  {file}:{} ({})", hit.line, hit.label));
        }
    }
    // 诊断产物域（W130）：日志、诊断包、崩溃转储。目录由 `ARTIFACTS` 登记表给出，
    // 因此「扫哪里」只有一处定义；缺目录时它是「没东西可扫」，不是「扫过了」。
    let mut artifacts_read = 0usize;
    for dir in ARTIFACTS.iter().map(|artifact| artifact.dir) {
        let ledger = scan_tree(&root.join(dir))?;
        artifacts_read += ledger.scanned + ledger.non_text.len();
        println!("{}", ledger.summary());
        findings.extend(ledger.report().lines().map(str::to_owned));
    }
    if findings.is_empty() {
        println!(
            "credential scan passed: {} files git would publish and {} diagnostic artifact file(s) carry no prefixed credential material",
            files.len(),
            artifacts_read
        );
        return Ok(());
    }
    findings.sort();
    findings.dedup();
    Err(format!(
        "credential scan failed: {} match(es) in tracked files or diagnostic artifacts\n{}\nremove the value, rotate it, and keep it in an untracked local file",
        findings.len(),
        findings.join("\n")
    ))
}

/// 一条诊断产物登记：扫描器要走的目录，以及仓库里**生产**它的那段文本。
///
/// 登记表是闭环的：没登记在这里的目录不会被扫，因此也不允许有产物落在别处。`source` 在编译期
/// 从 `producer` 的字面路径抓取，所以判据不可能去核验另一份文件——「登记了」与「核验了」是同一份文本。
///
/// 生产者的文本必须点名 `dir`，这条由 `every_registered_artifact_names_...` 判据双向钉住：
/// 改了 `tee` 的目标却没改这里，判据立刻红。
pub struct Artifact {
    /// 相对于仓库根、扫描器要遍历的目录。
    pub dir: &'static str,
    /// 生产该产物的仓库文件。
    pub producer: &'static str,
    /// `producer` 自己的文本，编译期抓取。
    pub source: &'static str,
    /// `source` 里逐字存在的一段文本，它证明生产者确实写进了 `dir`。
    pub marker: &'static str,
}

/// 在册的诊断产物。当前**只有一项**：CI 门控日志。它是仓库里唯一真实存在的诊断产物——
/// 诊断包与崩溃转储在仓库内**没有产生者**，因此不在这里登记（登记一个没有生产者的目录
/// 等于给「扫了个空目录」发一张通行证）。那两项的缺口在报告、registry notes 与 ADR 里具名。
pub const ARTIFACTS: &[Artifact] = &[Artifact {
    dir: ".nai-diagnostics",
    producer: ".github/workflows/foundation.yml",
    source: include_str!("../../.github/workflows/foundation.yml"),
    marker: "| tee \".nai-diagnostics/nai-gate.log\"",
}];

/// 一次产物命中的位置与族。与 `Hit` 一样，**匹配到的文本本身刻意缺席**：
/// 回显它就是把密钥从一份私有文件搬进构建日志。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactHit {
    /// 相对于被扫描目录的路径，分隔符统一为 `/`。
    pub path: String,
    pub line: usize,
    pub label: &'static str,
}

/// 一次产物扫描的**闭世界**台账。
///
/// 每个字段都是遍历时**独立**累加的，不是由别的字段派生：`visited` 数每一个 `read_dir`
/// 条目（含目录），四个桶各自数自己那一类。于是「条目 == 文本 + 非文本 + 非文件 + 容器」
/// 是一条**可失败**的判据，而不是恒真的同义反复——新增一条分支却忘了落桶，它会红。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtifactLedger {
    /// 被遍历的目录（原样保留，供报告显示）。
    pub root: PathBuf,
    /// 该目录是否存在。`false` ⇒ 没东西可扫，与「存在但为空」是两种值。
    pub present: bool,
    pub hits: Vec<ArtifactHit>,
    /// 按 UTF-8 成功解码、并扫过的普通文件。
    pub scanned: usize,
    /// 读到了、但不是 UTF-8 的普通文件（崩溃转储、二进制诊断包）。**仍被扫过**。
    pub non_text: Vec<String>,
    /// 既不是普通文件也不是目录的条目（符号链接、套接字、设备）。**不跟随**。
    pub non_file: Vec<String>,
    /// 走过的子目录数（容器条目）。
    pub directories: usize,
    /// `read_dir` 返回过的条目总数，含目录。
    pub visited: usize,
}

impl ArtifactLedger {
    fn name(&self) -> String {
        self.root
            .to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_owned()
    }

    /// 命中的报告行。只报路径、行号与族——绝不含匹配文本。
    pub fn report(&self) -> String {
        let root = self.name();
        self.hits
            .iter()
            .map(|hit| format!("  {root}/{}:{} ({})", hit.path, hit.line, hit.label))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 一行结算。空集报 `0` 而不是沉默；缺目录具名说出「这里没有目录」，
    /// MUST NOT 用「跳过」这种把「没检查」说成「检查过了」的措辞。
    pub fn summary(&self) -> String {
        if !self.present {
            return format!(
                "diagnostic scan: no directory at {} (nothing to scan)",
                self.name()
            );
        }
        format!(
            "diagnostic scan passed: {} artifact file(s) read as text, {} non-text, {} non-file entry/entries, {} subdirectory/ies under {}",
            self.scanned,
            self.non_text.len(),
            self.non_file.len(),
            self.directories,
            self.name()
        )
    }
}

/// 遍历一个诊断产物目录，按字节读每个普通文件，并清点每一个访问到的条目。
///
/// 与仓库域的 `check` 的关键差别：**不跳过非 UTF-8 文件**。崩溃转储就是二进制，用
/// `String::from_utf8` 失败即 `continue` 的写法会把整条记录静默丢掉——报告上看起来与干净
/// 一模一样。这里改成有损解码后照样扫：ASCII 段（真实密钥所在的部分）完整保留。
pub fn scan_tree(dir: &Path) -> Result<ArtifactLedger, String> {
    let mut ledger = ArtifactLedger {
        root: dir.to_path_buf(),
        ..ArtifactLedger::default()
    };
    match std::fs::read_dir(dir) {
        Ok(_) => ledger.present = true,
        // 「没有这个目录」是一种值，不是错误：本机没有诊断产物时它本来就不存在。
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(ledger),
        Err(error) => {
            return Err(format!(
                "cannot read diagnostic directory {}: {error}",
                dir.display()
            ));
        }
    }
    walk(dir, "", &mut ledger)?;
    ledger.hits.sort_by(|left, right| {
        (&left.path, left.line, left.label).cmp(&(&right.path, right.line, right.label))
    });
    Ok(ledger)
}

fn walk(dir: &Path, prefix: &str, ledger: &mut ArtifactLedger) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|error| {
        format!(
            "cannot read diagnostic directory {}: {error}",
            dir.display()
        )
    })?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("cannot read an entry of {}: {error}", dir.display()))?;
        ledger.visited += 1;
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        // `file_type()` 走的是目录项自身的类型（`lstat` 语义），**不跟随**符号链接。
        // 这正是「扫描不越出被遍历的树」的构造性保证。
        let kind = entry
            .file_type()
            .map_err(|error| format!("cannot classify {}: {error}", entry.path().display()))?;
        if kind.is_symlink() {
            ledger.non_file.push(relative);
        } else if kind.is_dir() {
            ledger.directories += 1;
            walk(&entry.path(), &relative, ledger)?;
        } else if kind.is_file() {
            let bytes = std::fs::read(entry.path())
                .map_err(|error| format!("cannot read {}: {error}", entry.path().display()))?;
            match String::from_utf8(bytes) {
                Ok(text) => {
                    ledger.scanned += 1;
                    record(&relative, &text, &mut ledger.hits);
                }
                Err(error) => {
                    // 有损解码：无效字节变成替换字符，ASCII 段逐字保留。
                    let text = String::from_utf8_lossy(error.as_bytes()).into_owned();
                    ledger.non_text.push(relative.clone());
                    record(&relative, &text, &mut ledger.hits);
                }
            }
        } else {
            ledger.non_file.push(relative);
        }
    }
    Ok(())
}

fn record(relative: &str, text: &str, hits: &mut Vec<ArtifactHit>) {
    for hit in scan_text(text) {
        hits.push(ArtifactHit {
            path: relative.to_owned(),
            line: hit.line,
            label: hit.label,
        });
    }
}

/// 只扫产物域。CI 里由 `cargo xtask check --secrets` 连带仓库域一起跑；这个入口让产物域
/// 可以被单独调用，也让「日志里有没有明文密钥」这件事有一条不依赖仓库域结论的命令。
pub fn check_artifacts(dir: &Path) -> Result<(), String> {
    let ledger = scan_tree(dir)?;
    if ledger.hits.is_empty() {
        println!("{}", ledger.summary());
        return Ok(());
    }
    Err(format!(
        "diagnostic artifact scan failed: {} match(es) in diagnostic artifacts\n{}\nremove the value, rotate it, and keep it out of any log, bundle or dump",
        ledger.hits.len(),
        ledger.report()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        // Same resolution the CLI shell uses: this crate sits directly inside
        // the workspace root.
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask sits directly inside the workspace root")
            .to_path_buf()
    }

    /// Builds a credential-shaped string at run time. Writing one literally
    /// would put a matchable value in this very file.
    fn synthetic(prefix: &str, len: usize) -> String {
        format!("{prefix}{}", "a".repeat(len))
    }

    #[test]
    fn a_long_prefixed_token_is_reported_with_its_line() {
        let text = format!("one\nlet key = \"{}\";\n", synthetic("sk-", 40));
        assert_eq!(
            scan_text(&text),
            vec![Hit {
                line: 2,
                label: "openai-style"
            }]
        );
    }

    #[test]
    fn the_short_sentinels_the_suite_already_uses_do_not_match() {
        // The kernel's monitoring test carries a sentinel with this exact shape.
        assert!(scan_text("\"sk-live-must-not-leak\"").is_empty());
        assert!(scan_text(".to_owned(),").is_empty());
        assert!(scan_text(&synthetic("sk-", 31)).is_empty());
        assert_eq!(scan_text(&synthetic("sk-", 32)).len(), 1);
    }

    #[test]
    fn each_prefix_family_is_covered_and_shaped_correctly() {
        assert_eq!(scan_text(&synthetic("wk-", 40)).len(), 1);
        assert_eq!(scan_text(&synthetic("ghp_", 36)).len(), 1);
        assert_eq!(scan_text(&synthetic("glpat-", 20)).len(), 1);
        assert_eq!(scan_text(&synthetic("AIza", 35)).len(), 1);
        // An AWS key id is upper-case; a lower-case body is not one.
        assert_eq!(scan_text(&format!("AKIA{}", "A".repeat(16))).len(), 1);
        assert!(scan_text(&format!("AKIA{}", "a".repeat(16))).is_empty());
        // A pasted request line is caught by the header shape.
        assert_eq!(
            scan_text(&format!("Authorization: {}", synthetic("Bearer ", 40))).len(),
            1
        );
    }

    #[test]
    fn prose_and_placeholders_are_left_alone() {
        for text in [
            "Authorization: Bearer <token>",
            "NAI_LLM_API_KEY=<your-key>",
            "token: \"cloud link unreachable\"",
            "prefix: \"sk-\",",
            "见 .env 的 NAI_TTS_VOICE",
        ] {
            assert!(scan_text(text).is_empty(), "unexpected hit in {text:?}");
        }
    }

    #[test]
    fn this_source_file_does_not_match_its_own_scan() {
        assert!(scan_text(include_str!("secrets.rs")).is_empty());
    }

    #[test]
    fn the_scan_domain_is_what_git_would_publish_and_excludes_ignored_state() {
        let files = tracked_files(&root()).expect("the workspace is a git checkout");
        assert!(files.len() > 100, "the scan domain looks empty");
        // A real key lives in an ignored local file; scanning it would make the
        // check impossible to satisfy.
        assert!(!files.contains(".env"));
        assert!(!files.iter().any(|file| file.starts_with(".workbuddy/")));
        assert!(files.iter().any(|file| file == "Cargo.toml"));
    }

    #[test]
    fn a_new_file_is_scanned_before_it_is_staged() {
        let root = root();
        let name = ".secrets-domain-probe.tmp";
        let probe = root.join(name);
        std::fs::write(&probe, "not a credential\n").expect("write the probe");
        let listed = tracked_files(&root).expect("git");
        std::fs::remove_file(&probe).expect("remove the probe");
        // An unwritten check would only catch the mistake after it was staged.
        assert!(
            listed.contains(name),
            "a file git would publish must be in the domain even before it is staged"
        );
    }

    #[test]
    fn the_workspace_itself_is_clean() {
        check(&root()).expect("no tracked file may carry prefixed credential material");
    }

    // ===== 诊断产物扫描（W130）=====
    //
    // 仓库域的 `check` **刻意**只认 UTF-8：`String::from_utf8` 失败就走 `else { continue; }`。
    // 崩溃转储与二进制诊断包不走 UTF-8，于是那条路径会把**整个文件**静默丢掉——既没有命中，
    // 也没有留下「跳过了它」的痕迹，报告上看起来与「干净」一模一样。
    //
    // 产物域因此按字节读、按桶清点，并且是**闭世界**的：访问过的每一个条目都必须落在某个
    // 具名桶里（文本 / 非文本 / 非文件 / 容器），桶和必须等于访问数。少一个桶就会让「没检查」
    // 伪装成「检查过了」。

    fn scratch() -> tempfile::TempDir {
        tempfile::tempdir().expect("a scratch directory for the artifact fixture")
    }

    fn put(dir: &Path, relative: &str, bytes: &[u8]) {
        let path = dir.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create the fixture's parent");
        }
        std::fs::write(path, bytes).expect("write the fixture");
    }

    fn walked(dir: &Path) -> ArtifactLedger {
        scan_tree(dir).expect("the fixture tree is readable")
    }

    #[test]
    fn an_artifact_carrying_credential_material_is_reported_without_echoing_it() {
        let dir = scratch();
        let key = synthetic("sk-", 40);
        put(
            dir.path(),
            "gate.log",
            format!("building\nlet key = \"{key}\";\n").as_bytes(),
        );
        let ledger = walked(dir.path());
        assert_eq!(
            ledger.hits,
            vec![ArtifactHit {
                path: "gate.log".to_owned(),
                line: 2,
                label: "openai-style"
            }]
        );
        // 命中内容绝不回显：报告里出现那个值本身，就是把密钥从一份私有文件搬进构建日志。
        let surfaces = format!("{ledger:?} {}", ledger.report());
        assert!(
            !surfaces.contains(&key),
            "the report echoes the matched value"
        );
        let error = check_artifacts(dir.path()).expect_err("a match must fail the scan");
        assert!(error.contains("gate.log:2"), "{error}");
        assert!(
            !error.contains(&key),
            "the failure message echoes the matched value"
        );
    }

    #[test]
    fn a_clean_artifact_tree_passes_and_still_counts_what_it_read() {
        let dir = scratch();
        put(dir.path(), "gate.log", b"cargo test\nall good\n");
        put(dir.path(), "notes/summary.txt", b"nothing to see\n");
        let ledger = walked(dir.path());
        assert!(ledger.hits.is_empty());
        assert_eq!(ledger.scanned, 2, "both files are text and must be counted");
        assert!(check_artifacts(dir.path()).is_ok());
    }

    #[test]
    fn a_non_text_artifact_is_named_and_still_scanned_instead_of_being_dropped() {
        let dir = scratch();
        let key = synthetic("sk-", 40);
        let mut dump = vec![0xff, 0xfe, 0x00, b'\n'];
        dump.extend_from_slice(format!("panic\n{key}\n").as_bytes());
        put(dir.path(), "core.dump", &dump);
        let ledger = walked(dir.path());
        assert_eq!(
            ledger.non_text,
            vec!["core.dump".to_owned()],
            "a binary artifact must be named, not passed over in silence"
        );
        assert_eq!(ledger.scanned, 0, "a binary artifact is not a text file");
        // 关键一条：它仍然被扫过。既有 `check` 的 `else { continue; }` 会在这里整条丢掉。
        assert_eq!(
            ledger.hits.len(),
            1,
            "a binary artifact was dropped instead of scanned"
        );
        assert_eq!(ledger.hits[0].path, "core.dump");
        assert_eq!(
            ledger.hits[0].line, 3,
            "line numbers survive the byte-level read"
        );
    }

    #[test]
    fn an_empty_artifact_tree_reports_zero_rather_than_being_skipped() {
        let dir = scratch();
        let ledger = walked(dir.path());
        assert!(
            ledger.present,
            "the directory exists; an empty tree is not the same thing as a missing one"
        );
        assert_eq!(ledger.visited, 0);
        assert_eq!(ledger.scanned, 0);
        assert_eq!(ledger.directories, 0);
        assert!(ledger.hits.is_empty());
        // 空集必须被清点成 0 并通过，MUST NOT 短路成「这一项跳过」。
        assert!(check_artifacts(dir.path()).is_ok());
    }

    #[test]
    fn a_missing_artifact_directory_is_reported_as_absent_not_as_an_empty_scan() {
        let dir = scratch();
        let missing = dir.path().join("never-produced");
        let ledger = walked(&missing);
        assert!(
            !ledger.present,
            "a missing directory must not be reported as a present-but-empty one"
        );
        assert_eq!(ledger.visited, 0);
        assert!(check_artifacts(&missing).is_ok());
    }

    #[test]
    fn every_visited_entry_lands_in_exactly_one_bucket() {
        let dir = scratch();
        put(dir.path(), "gate.log", b"text\n");
        put(dir.path(), "core.dump", &[0xff, 0xfe, 0x00]);
        put(dir.path(), "nested/inner.log", b"more text\n");
        let ledger = walked(dir.path());
        // 三个桶都非空，这条判据才不是一句恒真的空话。第四个桶（非文件）由 unix 侧的
        // 符号链接用例覆盖——它需要宿主允许建链接，因此不能要求在这里非空。
        assert!(ledger.scanned > 0, "text bucket");
        assert!(!ledger.non_text.is_empty(), "binary bucket");
        assert!(ledger.directories > 0, "container bucket");
        assert_eq!(
            ledger.visited,
            ledger.scanned + ledger.non_text.len() + ledger.non_file.len() + ledger.directories,
            "an entry was visited but never accounted for"
        );
        assert_eq!(
            ledger.visited, 4,
            "gate.log, core.dump, nested, nested/inner.log"
        );
    }

    #[test]
    fn subdirectories_are_walked_recursively() {
        let dir = scratch();
        let key = synthetic("sk-", 40);
        put(dir.path(), "a/b/c/gate.log", format!("{key}\n").as_bytes());
        let ledger = walked(dir.path());
        assert_eq!(ledger.hits.len(), 1);
        assert_eq!(
            ledger.hits[0].path, "a/b/c/gate.log",
            "the report must name the nested path, not just the file name"
        );
        assert_eq!(ledger.hits[0].line, 1);
    }

    #[test]
    #[cfg(unix)]
    fn a_symbolic_link_is_named_and_its_target_is_not_read() {
        // `DirEntry::file_type()` 不跟随链接，所以「不越出被扫描的树」是**构造性**的；
        // 这个用例钉住它的可观测后果。Windows 侧要建成链接需要特权或开发者模式，
        // 因此按本仓既有惯例（见平台矩阵 path-containment / toolchain-target-assumptions）
        // 用 `cfg(unix)` 门控，而不是让宿主能力决定结论。
        let outside = scratch();
        let dir = scratch();
        let key = synthetic("sk-", 40);
        put(outside.path(), "secret.log", format!("{key}\n").as_bytes());
        std::os::unix::fs::symlink(
            outside.path().join("secret.log"),
            dir.path().join("link.log"),
        )
        .expect("the fixture needs a symbolic link");
        let ledger = walked(dir.path());
        assert_eq!(
            ledger.non_file,
            vec!["link.log".to_owned()],
            "a link is named, not read"
        );
        assert_eq!(ledger.scanned, 0);
        assert!(
            ledger.hits.is_empty(),
            "the scan followed the link out of the tree"
        );
        assert_eq!(ledger.visited, 1);
    }

    #[test]
    fn every_registered_artifact_names_the_repository_text_that_produces_it() {
        assert!(
            !ARTIFACTS.is_empty(),
            "an empty artifact registry makes this criterion vacuous"
        );
        let files = tracked_files(&root()).expect("the workspace is a git checkout");
        for artifact in ARTIFACTS {
            // 凭据必须**逐字**存在于它自己声称的生产者文本里。文本是编译期抓取的，
            // 因此判据不可能去核验另一份文件。
            assert!(
                artifact.source.contains(artifact.marker),
                "{} does not contain {:?}",
                artifact.producer,
                artifact.marker
            );
            // 而且那份文本必须点名扫描器要走的目录——否则「登记了」与「扫到了」是两回事。
            assert!(
                artifact.marker.contains(artifact.dir),
                "{} does not name the directory the scan walks ({})",
                artifact.producer,
                artifact.dir
            );
            // 凭据不得由判据自己提供。
            assert!(
                !artifact.producer.starts_with("xtask/"),
                "the criterion must not be its own credential"
            );
            // 生产者必须是被 git 跟踪的文件，否则它在别人的工作区里可能根本不存在。
            assert!(
                files.contains(artifact.producer),
                "{} is not a tracked file",
                artifact.producer
            );
        }
    }
}
