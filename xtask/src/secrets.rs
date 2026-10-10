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
use std::path::Path;
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
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        for hit in scan_text(&text) {
            findings.push(format!("  {file}:{} ({})", hit.line, hit.label));
        }
    }
    if findings.is_empty() {
        println!(
            "credential scan passed: {} files git would publish, no prefixed credential material",
            files.len()
        );
        return Ok(());
    }
    findings.sort();
    findings.dedup();
    Err(format!(
        "credential scan failed: {} match(es) in tracked files\n{}\nremove the value, rotate it, and keep it in an untracked local file",
        findings.len(),
        findings.join("\n")
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
}
