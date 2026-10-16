//! Download-phase checks: interruption, resumption, bounded re-asking and the
//! hash gate — plus one run over a real loopback TCP/HTTP/1.1 pair.
//!
//! Two kinds of evidence live here on purpose. The scripted transport makes the
//! fine-grained facts cheap to state (exact window offsets, exact attempt
//! counts, a ledger that lies). The loopback peer makes one of them real: the
//! resume offset is asserted against the `Range` header that actually went out
//! on a socket, not against the transport's own bookkeeping.
use nai_contracts::v1::ErrorCode;
use nai_security::Operation;
use nai_upgrade_core::{
    DeterministicVerifier, DownloadRequest, FetchPolicy, FetchWindow, Fetched, PackageFetcher,
    ResumeLedger, RevocationList, UpgradeError, VerifyLimits, download_package,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Mutex, OnceLock, atomic::AtomicBool},
    time::{Duration, Instant},
};

/// A fixed cancellation flag: these checks never cancel, and the download's own
/// stop channel is the `Operation`.
fn cancelled() -> &'static AtomicBool {
    static FLAG: OnceLock<AtomicBool> = OnceLock::new();
    FLAG.get_or_init(|| AtomicBool::new(false))
}

fn op() -> Operation {
    Operation::new(Duration::from_secs(20)).expect("budget within the ceiling")
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(25)
}

fn open_revocations(now_ms: i64) -> RevocationList {
    RevocationList::new(now_ms, now_ms + 60_000, []).expect("a fresh cache")
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The artifact every check downloads: deterministic, non-trivial, and long
/// enough that a half of it is a meaningful resume point.
fn content() -> Vec<u8> {
    (0..4096u32).map(|i| (i % 251) as u8).collect()
}

const ARTIFACT: &str = "bin/app";

/// A schema-valid manifest declaring exactly `files`.
fn manifest_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let declared: Vec<Value> = files
        .iter()
        .map(|(path, content)| {
            json!({ "path": path, "sha256": sha256_hex(content), "bytes": content.len().to_string() })
        })
        .collect();
    serde_json::to_vec(&json!({
        "manifest_schema": 1,
        "package_id": "plugin.demo",
        "package_version": "1.2.3",
        "kind": "plugin",
        "kernel_range": ">=1.0.0, <2.0.0",
        "requires": [{ "ref": "cap.one", "range": ">=1.0.0, <2.0.0" }],
        "hosts": ["inproc"],
        "os": ["linux"],
        "architectures": ["x86_64"],
        "runtime": [{ "id": "rt-cpu", "accel": "cpu" }],
        "files": declared,
        "install_bytes": "1024",
        "migrated_bytes": "2048",
        "signer_ref": "signer.demo",
        "license_ref": "license.demo",
        "migration_ref": "migration.forward",
        "rollback_range": ">=1.0.0, <1.2.0",
        "cost": { "currency": "CNY", "cloud": "10", "download": null, "storage": "5", "runtime_delta": null }
    }))
    .expect("a JSON manifest")
}

/// The transaction root for the common case: one artifact, the declared bytes.
struct Fixture {
    manifest: Vec<u8>,
    signature: Vec<u8>,
    revocations: RevocationList,
}
impl Fixture {
    fn new(content: &[u8]) -> Self {
        let manifest = manifest_of(&[(ARTIFACT, content)]);
        let signature = DeterministicVerifier::sign(&manifest);
        Self {
            manifest,
            signature,
            revocations: open_revocations(1_000),
        }
    }
    fn request(&self, policy: FetchPolicy, limits: VerifyLimits) -> DownloadRequest<'_> {
        DownloadRequest {
            manifest: &self.manifest,
            signature: &self.signature,
            verifier: &DeterministicVerifier,
            revocations: &self.revocations,
            now_ms: 1_000,
            policy,
            limits,
            deadline: deadline(),
            cancelled: cancelled(),
        }
    }
}

/// A transport that answers from a script, recording the window it was asked
/// for. The record is the point: "the offset came from the ledger" is asserted
/// against what the transport was actually handed.
struct ScriptedFetcher {
    script: VecDeque<Result<Vec<u8>, UpgradeError>>,
    asked: Vec<(String, u64, u64)>,
}
impl ScriptedFetcher {
    fn new(script: Vec<Result<Vec<u8>, UpgradeError>>) -> Self {
        Self {
            script: script.into(),
            asked: Vec::new(),
        }
    }
    /// Deliver `bytes` for the next window.
    fn deliver<const N: usize>(streams: [Vec<u8>; N]) -> Self {
        Self::new(streams.into_iter().map(Ok).collect())
    }
}
impl PackageFetcher for ScriptedFetcher {
    fn fetch(
        &mut self,
        _op: &Operation,
        window: &FetchWindow<'_>,
    ) -> Result<Fetched, UpgradeError> {
        self.asked
            .push((window.path.to_owned(), window.offset, window.length));
        let next = self
            .script
            .pop_front()
            .expect("the download asked for more windows than the script allows");
        next.map(|bytes| Fetched { bytes })
    }
}

/// An in-memory ledger. `swallow` makes `append` report success while storing
/// nothing, which is the seam failing its contract: the download MUST notice.
#[derive(Default)]
struct MemoryLedger {
    files: BTreeMap<String, Vec<u8>>,
    swallow: bool,
    discards: usize,
}
impl MemoryLedger {
    fn prefilled(path: &str, bytes: &[u8]) -> Self {
        let mut ledger = Self::default();
        ledger.files.insert(path.to_owned(), bytes.to_vec());
        ledger
    }
}
impl ResumeLedger for MemoryLedger {
    fn durable_bytes(&self, path: &str) -> u64 {
        self.files.get(path).map_or(0, |bytes| bytes.len() as u64)
    }
    fn append(
        &mut self,
        _op: &Operation,
        path: &str,
        offset: u64,
        chunk: &[u8],
    ) -> Result<(), UpgradeError> {
        if self.swallow {
            return Ok(());
        }
        let entry = self.files.entry(path.to_owned()).or_default();
        assert_eq!(
            entry.len() as u64,
            offset,
            "a ledger append must continue the durable prefix, not land anywhere"
        );
        entry.extend_from_slice(chunk);
        Ok(())
    }
    fn assembled(&self, path: &str) -> Vec<u8> {
        self.files.get(path).cloned().unwrap_or_default()
    }
    fn discard(&mut self, _op: &Operation, path: &str) -> Result<(), UpgradeError> {
        self.files.remove(path);
        self.discards += 1;
        Ok(())
    }
}

fn failure(code: ErrorCode) -> Result<Vec<u8>, UpgradeError> {
    Err(UpgradeError {
        code,
        reason: "scripted transport failure",
    })
}

// ---------------------------------------------------------------------------
// Resumption
// ---------------------------------------------------------------------------

#[test]
fn a_short_read_resumes_from_the_durable_offset() {
    let content = content();
    let fixture = Fixture::new(&content);
    // First attempt: half the artifact, then the stream dies.
    let mut fetcher =
        ScriptedFetcher::deliver([content[..1024].to_vec(), content[1024..].to_vec()]);
    let mut ledger = MemoryLedger::default();

    let report = download_package(
        &op(),
        &fixture.request(FetchPolicy::default(), VerifyLimits::default()),
        &mut fetcher,
        &mut ledger,
    )
    .expect("an interrupted stream is survived by resuming");

    assert_eq!(
        fetcher.asked,
        vec![
            (ARTIFACT.to_owned(), 0, 4096),
            (ARTIFACT.to_owned(), 1024, 3072),
        ],
        "the second window must start where the durable prefix ends"
    );
    assert_eq!(report.windows_requested, 2);
    assert_eq!(report.interruptions, 1, "the short read must be counted");
    assert_eq!(report.resumed_bytes, 0, "nothing was durable at the start");
    assert_eq!(report.verified.content_bytes, 4096);
}

#[test]
fn a_resume_point_left_by_a_previous_run_is_honoured() {
    let content = content();
    let fixture = Fixture::new(&content);
    // What a previous process left durable on local media.
    let mut ledger = MemoryLedger::prefilled(ARTIFACT, &content[..1024]);
    let mut fetcher = ScriptedFetcher::deliver([content[1024..].to_vec()]);

    let report = download_package(
        &op(),
        &fixture.request(FetchPolicy::default(), VerifyLimits::default()),
        &mut fetcher,
        &mut ledger,
    )
    .expect("a prior partial download is resumed, not restarted");

    assert_eq!(
        fetcher.asked,
        vec![(ARTIFACT.to_owned(), 1024, 3072)],
        "not one byte of the durable prefix may be asked for again"
    );
    assert_eq!(report.resumed_bytes, 1024);
    assert_eq!(report.interruptions, 0);
}

#[test]
fn an_artifact_already_durable_is_verified_without_moving_a_byte() {
    let content = content();
    let fixture = Fixture::new(&content);
    let mut ledger = MemoryLedger::prefilled(ARTIFACT, &content);
    // No script entries: asking for a window here fails the test.
    let mut fetcher = ScriptedFetcher::new(Vec::new());

    let report = download_package(
        &op(),
        &fixture.request(FetchPolicy::default(), VerifyLimits::default()),
        &mut fetcher,
        &mut ledger,
    )
    .expect("the content is already local");

    assert_eq!(report.windows_requested, 0);
    assert_eq!(report.resumed_bytes, 4096);
    // The artifact verified even though this run moved no bytes, so the hash
    // gate's input came from the ledger rather than from a buffer this run holds.
    assert_eq!(report.verified.content_bytes, 4096);
}

#[test]
fn an_oversized_resume_point_is_refused_not_clamped() {
    let content = content();
    let fixture = Fixture::new(&content);
    let mut ledger = MemoryLedger::prefilled(ARTIFACT, &[vec![0u8; 4097], content].concat());
    let mut fetcher = ScriptedFetcher::new(Vec::new());

    let error = download_package(
        &op(),
        &fixture.request(FetchPolicy::default(), VerifyLimits::default()),
        &mut fetcher,
        &mut ledger,
    )
    .expect_err("a resume point past the declared end describes another artifact");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert_eq!(error.reason, "resume ledger exceeds declared size");
    assert!(fetcher.asked.is_empty(), "nothing may be requested");
}

#[test]
fn a_ledger_that_does_not_persist_is_a_named_failure() {
    let content = content();
    let fixture = Fixture::new(&content);
    let mut fetcher =
        ScriptedFetcher::deliver([content[..1024].to_vec(), content[1024..].to_vec()]);
    let mut ledger = MemoryLedger {
        swallow: true,
        ..MemoryLedger::default()
    };

    let error = download_package(
        &op(),
        &fixture.request(FetchPolicy::default(), VerifyLimits::default()),
        &mut fetcher,
        &mut ledger,
    )
    .expect_err("a chunk that never became durable is not progress");
    assert_eq!(error.code, ErrorCode::Internal);
    assert_eq!(error.reason, "resume ledger did not persist the chunk");
    assert_eq!(
        fetcher.asked.len(),
        1,
        "the first window must not be re-asked as if it had landed"
    );
}

// ---------------------------------------------------------------------------
// The re-ask budget
// ---------------------------------------------------------------------------

#[test]
fn the_default_budget_is_the_declared_amplitude() {
    // 幅度判据：声明「默认每窗口 3 次尝试」，这里逐项断言常量就是那个幅度。
    assert_eq!(FetchPolicy::default().max_attempts_per_window, 3);
}

#[test]
fn a_short_read_at_the_budget_boundary_still_succeeds() {
    let content = content();
    let fixture = Fixture::new(&content);
    let policy = FetchPolicy {
        max_attempts_per_window: 2,
    };
    // Two attempts, the second of which completes: exactly at the bound.
    let mut fetcher =
        ScriptedFetcher::deliver([content[..1024].to_vec(), content[1024..].to_vec()]);
    let mut ledger = MemoryLedger::default();

    let report = download_package(
        &op(),
        &fixture.request(policy, VerifyLimits::default()),
        &mut fetcher,
        &mut ledger,
    )
    .expect("the last allowed attempt may still succeed");
    assert_eq!(
        report.windows_requested, 2,
        "the attempt count is pinned, not just the outcome"
    );
}

#[test]
fn one_attempt_past_the_budget_is_refused_by_name() {
    let content = content();
    let fixture = Fixture::new(&content);
    let policy = FetchPolicy {
        max_attempts_per_window: 2,
    };
    // A source that keeps dying: every attempt is a short read.
    let mut fetcher = ScriptedFetcher::deliver([
        content[..512].to_vec(),
        content[512..1024].to_vec(),
        content[1024..1536].to_vec(),
    ]);
    let mut ledger = MemoryLedger::default();

    let error = download_package(
        &op(),
        &fixture.request(policy, VerifyLimits::default()),
        &mut fetcher,
        &mut ledger,
    )
    .expect_err("a source that never completes must not loop forever");
    assert_eq!(error.code, ErrorCode::ResourceExhausted);
    assert_eq!(error.reason, "download retry budget exhausted");
    assert_eq!(
        fetcher.asked.len(),
        2,
        "exactly the budget's worth of attempts, then stop"
    );
}

#[test]
fn a_transport_error_is_final_and_not_re_asked() {
    let fixture = Fixture::new(&content());
    let mut fetcher = ScriptedFetcher::new(vec![failure(ErrorCode::PermissionDenied)]);
    let mut ledger = MemoryLedger::default();

    let error = download_package(
        &op(),
        &fixture.request(FetchPolicy::default(), VerifyLimits::default()),
        &mut fetcher,
        &mut ledger,
    )
    .expect_err("a transport error is the transport saying asking again will not help");
    assert_eq!(error.code, ErrorCode::PermissionDenied);
    assert_eq!(
        fetcher.asked.len(),
        1,
        "no retry may be spent on a final error"
    );
}

#[test]
fn more_than_the_window_is_refused() {
    let content = content();
    let fixture = Fixture::new(&content);
    let mut overshoot = content.clone();
    overshoot.push(0x7f);
    let mut fetcher = ScriptedFetcher::deliver([overshoot]);
    let mut ledger = MemoryLedger::default();

    let error = download_package(
        &op(),
        &fixture.request(FetchPolicy::default(), VerifyLimits::default()),
        &mut fetcher,
        &mut ledger,
    )
    .expect_err("bytes beyond the window would extend the artifact");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert_eq!(error.reason, "download returned more than requested");
}

#[test]
fn a_source_that_delivers_nothing_is_re_asked_then_named() {
    let fixture = Fixture::new(&content());
    let policy = FetchPolicy {
        max_attempts_per_window: 1,
    };
    let mut fetcher = ScriptedFetcher::deliver([Vec::new()]);
    let mut ledger = MemoryLedger::default();

    let error = download_package(
        &op(),
        &fixture.request(policy, VerifyLimits::default()),
        &mut fetcher,
        &mut ledger,
    )
    .expect_err("zero bytes is not completion");
    assert_eq!(error.code, ErrorCode::ResourceExhausted);
    assert_eq!(fetcher.asked.len(), 1);
}

// ---------------------------------------------------------------------------
// The hash gate
// ---------------------------------------------------------------------------

#[test]
fn a_complete_but_corrupt_artifact_is_refused_and_discarded() {
    let content = content();
    let fixture = Fixture::new(&content);
    let mut corrupt = content.clone();
    corrupt[2048] ^= 0xff;
    let mut fetcher = ScriptedFetcher::deliver([corrupt]);
    let mut ledger = MemoryLedger::default();

    let error = download_package(
        &op(),
        &fixture.request(FetchPolicy::default(), VerifyLimits::default()),
        &mut fetcher,
        &mut ledger,
    )
    .expect_err("bytes that do not hash to the signed declaration are not the package");
    assert_eq!(
        error.code,
        ErrorCode::InvalidArgument,
        "the same token verify_package uses: one verification chain, not two"
    );
    assert_eq!(error.reason, "package content hash mismatch");
    assert_eq!(
        ledger.durable_bytes(ARTIFACT),
        0,
        "a corrupt artifact must not survive as the next run's resume point"
    );
    assert_eq!(ledger.discards, 1);
}

#[test]
fn a_discarded_corrupt_artifact_is_refetched_from_zero() {
    let content = content();
    let fixture = Fixture::new(&content);
    let corrupt = {
        let mut bytes = content.clone();
        bytes[64] ^= 0xff;
        bytes
    };

    // First run: the source hands back something corrupt.
    let mut ledger = MemoryLedger::default();
    let mut first = ScriptedFetcher::deliver([corrupt]);
    download_package(
        &op(),
        &fixture.request(FetchPolicy::default(), VerifyLimits::default()),
        &mut first,
        &mut ledger,
    )
    .expect_err("the corrupt artifact is refused");

    // Second run over the same ledger: with the corrupt prefix discarded the
    // download starts at zero again instead of resuming at the end of it.
    let mut second = ScriptedFetcher::deliver([content[..512].to_vec(), content[512..].to_vec()]);
    let report = download_package(
        &op(),
        &fixture.request(FetchPolicy::default(), VerifyLimits::default()),
        &mut second,
        &mut ledger,
    )
    .expect("the same artifact must be recoverable on the next run");

    assert_eq!(
        second.asked,
        vec![
            (ARTIFACT.to_owned(), 0, 4096),
            (ARTIFACT.to_owned(), 512, 3584),
        ],
        "the second run must not resume inside the bytes it just discarded"
    );
    assert_eq!(report.resumed_bytes, 0);
}

// ---------------------------------------------------------------------------
// Bounds, and what a download does not decide
// ---------------------------------------------------------------------------

#[test]
fn the_declared_bounds_are_applied_before_any_window() {
    let fixture = Fixture::new(&content());
    let mut fetcher = ScriptedFetcher::new(Vec::new());

    let too_many = download_package(
        &op(),
        &fixture.request(
            FetchPolicy::default(),
            VerifyLimits {
                max_files: 0,
                max_bytes: u64::MAX,
            },
        ),
        &mut fetcher,
        &mut MemoryLedger::default(),
    )
    .expect_err("the declared set exceeds the file bound");
    assert_eq!(too_many.code, ErrorCode::ResourceExhausted);
    assert_eq!(too_many.reason, "package file count limit");

    let too_big = download_package(
        &op(),
        &fixture.request(
            FetchPolicy::default(),
            VerifyLimits {
                max_files: 8,
                max_bytes: 1,
            },
        ),
        &mut fetcher,
        &mut MemoryLedger::default(),
    )
    .expect_err("the declared size exceeds the byte bound");
    assert_eq!(too_big.code, ErrorCode::ResourceExhausted);
    assert_eq!(too_big.reason, "package content byte limit");

    assert!(
        fetcher.asked.is_empty(),
        "a bound that is known from the declaration must cost nothing to enforce"
    );
}

#[test]
fn a_downloaded_artifact_still_has_to_pass_the_verification_chain() {
    let content = content();
    let fixture = Fixture::new(&content);
    let mut fetcher = ScriptedFetcher::deliver([content.clone()]);
    let mut ledger = MemoryLedger::default();
    // Same bytes, a signature that is not this manifest's.
    let mut request = fixture.request(FetchPolicy::default(), VerifyLimits::default());
    let wrong = DeterministicVerifier::sign(b"another manifest");
    request.signature = &wrong;

    let error = download_package(&op(), &request, &mut fetcher, &mut ledger)
        .expect_err("a download is not an acceptance");
    assert_eq!(error.code, ErrorCode::PermissionDenied);
    assert_eq!(
        fetcher.asked.len(),
        1,
        "the bytes did arrive; it is the verification chain that refused them"
    );
}

#[test]
fn a_cancelled_operation_does_no_work() {
    let fixture = Fixture::new(&content());
    let mut fetcher = ScriptedFetcher::new(Vec::new());
    let stopped = Operation::new(Duration::ZERO).expect("a zero budget is legal");
    stopped.cancel();

    let error = download_package(
        &stopped,
        &fixture.request(FetchPolicy::default(), VerifyLimits::default()),
        &mut fetcher,
        &mut MemoryLedger::default(),
    )
    .expect_err("a cancelled operation must not start transferring");
    assert_eq!(error.code, ErrorCode::Cancelled);
    assert!(fetcher.asked.is_empty());
}

/// A transport that cancels the operation the moment it is handed a window.
/// The stop signal therefore arrives *after* the entry check has already passed,
/// which is what makes this a construction rather than a timing race.
struct CancellingFetcher {
    asked: usize,
}
impl PackageFetcher for CancellingFetcher {
    fn fetch(
        &mut self,
        op: &Operation,
        _window: &FetchWindow<'_>,
    ) -> Result<Fetched, UpgradeError> {
        self.asked += 1;
        op.cancel();
        // The in-flight window dies with the operation.
        Ok(Fetched { bytes: Vec::new() })
    }
}

#[test]
fn a_cancellation_arriving_mid_transfer_stops_the_loop() {
    let fixture = Fixture::new(&content());
    let mut fetcher = CancellingFetcher { asked: 0 };
    let mut ledger = MemoryLedger::default();

    let error = download_package(
        &op(),
        &fixture.request(FetchPolicy::default(), VerifyLimits::default()),
        &mut fetcher,
        &mut ledger,
    )
    .expect_err("a cancel that arrives mid-transfer must stop the loop");
    assert_eq!(error.code, ErrorCode::Cancelled);
    assert_eq!(
        fetcher.asked, 1,
        "the window already in flight finishes; no further one may start"
    );
}

#[test]
fn the_callers_stop_signals_are_honoured_before_the_first_window() {
    let fixture = Fixture::new(&content());
    let mut fetcher = ScriptedFetcher::new(Vec::new());

    // The caller's own cancellation flag, which is not the operation's.
    let stop = AtomicBool::new(true);
    let mut request = fixture.request(FetchPolicy::default(), VerifyLimits::default());
    request.cancelled = &stop;
    let error = download_package(&op(), &request, &mut fetcher, &mut MemoryLedger::default())
        .expect_err("a caller-side cancellation must stop before any transfer");
    assert_eq!(error.code, ErrorCode::Cancelled);

    // A deadline that has already passed.
    let mut expired = fixture.request(FetchPolicy::default(), VerifyLimits::default());
    expired.deadline = Instant::now();
    let error = download_package(&op(), &expired, &mut fetcher, &mut MemoryLedger::default())
        .expect_err("an expired deadline must stop before any transfer");
    assert_eq!(error.code, ErrorCode::DeadlineExceeded);

    assert!(
        fetcher.asked.is_empty(),
        "a stop signal that costs a window is not a stop signal"
    );
}

// ---------------------------------------------------------------------------
// The same facts, over a real socket
// ---------------------------------------------------------------------------

mod loopback {
    //! A minimal HTTP/1.1 peer and client.
    //!
    //! Neither is a production downloader, and neither is meant to be: the point
    //! of this module is that the resume offset asserted in the checks above
    //! also holds when the window crosses a real socket. The peer speaks the
    //! wire protocol directly so it can deliver **fewer bytes than its own
    //! `Content-Length`** — an interrupted transfer, which a well-behaved
    //! library server would not let us stage. No HTTP dependency is added, so
    //! the crate's "no network dependency" statement stays literally true.
    use super::*;
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::{Shutdown, TcpListener, TcpStream},
        sync::Arc,
        thread,
    };

    pub struct Peer {
        pub port: u16,
        /// Every `Range: bytes=N-` the peer received, in order.
        pub ranges: Arc<Mutex<Vec<u64>>>,
    }
    impl Peer {
        /// Serve `body` forever. `drop[i]` is how many bytes response `i` sends:
        /// fewer than the window it declares, which is what an interrupted
        /// download looks like on the wire. Responses past the end of `drop`
        /// send their whole window.
        pub fn start(body: Vec<u8>, drop: Vec<usize>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("an ephemeral port");
            let port = listener.local_addr().expect("a bound address").port();
            let ranges = Arc::new(Mutex::new(Vec::new()));
            let recorded = ranges.clone();
            thread::spawn(move || {
                for (response, stream) in listener.incoming().enumerate() {
                    let Ok(mut stream) = stream else { break };
                    let mut head = String::new();
                    if let Ok(clone) = stream.try_clone() {
                        let mut reader = BufReader::new(clone);
                        loop {
                            let mut line = String::new();
                            match reader.read_line(&mut line) {
                                Ok(0) | Err(_) => break,
                                Ok(_) if line == "\r\n" => break,
                                Ok(_) => head.push_str(&line),
                            }
                        }
                    }
                    let offset = head
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            if !name.eq_ignore_ascii_case("range") {
                                return None;
                            }
                            value
                                .trim()
                                .strip_prefix("bytes=")?
                                .trim_end_matches('-')
                                .parse()
                                .ok()
                        })
                        .unwrap_or(0u64);
                    recorded.lock().expect("the recorder lock").push(offset);
                    let remaining = &body[offset as usize..];
                    let declared = remaining.len();
                    let deliver = drop
                        .get(response)
                        .copied()
                        .unwrap_or(declared)
                        .min(declared);
                    let status = if offset == 0 {
                        "200 OK"
                    } else {
                        "206 Partial Content"
                    };
                    let header = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {declared}\r\n\
                         Content-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",
                        offset,
                        offset + declared as u64 - 1,
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(&remaining[..deliver]);
                    let _ = stream.flush();
                    let _ = stream.shutdown(Shutdown::Write);
                }
            });
            Self { port, ranges }
        }
    }

    /// The client: one real `GET` with a `Range` header per window. It returns
    /// exactly the body that arrived, which may be short.
    pub struct HttpFetcher {
        pub port: u16,
    }
    impl PackageFetcher for HttpFetcher {
        fn fetch(
            &mut self,
            _op: &Operation,
            window: &FetchWindow<'_>,
        ) -> Result<Fetched, UpgradeError> {
            let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("a connection");
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .expect("a read timeout");
            let request = format!(
                "GET /v1/packages/plugin.demo/1.2.3 HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\
                 Range: bytes={}-\r\nConnection: close\r\n\r\n",
                self.port, window.offset
            );
            stream
                .write_all(request.as_bytes())
                .expect("the request is written");
            let mut reply = Vec::new();
            stream.read_to_end(&mut reply).expect("a complete reply");
            let split = reply
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .expect("a header terminator");
            Ok(Fetched {
                bytes: reply[split + 4..].to_vec(),
            })
        }
    }
}

#[test]
fn an_interrupted_transfer_over_a_real_socket_resumes_and_verifies() {
    use loopback::{HttpFetcher, Peer};

    let content = content();
    let fixture = Fixture::new(&content);
    // The first response declares the whole window and sends a quarter of it.
    let peer = Peer::start(content.clone(), vec![1024]);
    let mut fetcher = HttpFetcher { port: peer.port };
    let mut ledger = MemoryLedger::default();

    let report = download_package(
        &op(),
        &fixture.request(FetchPolicy::default(), VerifyLimits::default()),
        &mut fetcher,
        &mut ledger,
    )
    .expect("an interrupted transfer is survived over a real socket");

    assert_eq!(report.windows_requested, 2);
    assert_eq!(report.interruptions, 1);
    assert_eq!(report.verified.content_bytes, 4096);
    // The wire is the evidence: the second window carried `Range: bytes=1024-`,
    // the offset the ledger reported. A build that resumed from zero, or from
    // its own bookkeeping, would send `0-` twice.
    assert_eq!(
        *peer.ranges.lock().expect("the recorder lock"),
        vec![0u64, 1024u64]
    );
}

#[test]
fn a_source_that_keeps_dropping_the_socket_is_refused_by_name() {
    use loopback::{HttpFetcher, Peer};

    let content = content();
    let fixture = Fixture::new(&content);
    // Every response declares its window and sends one byte of it.
    let peer = Peer::start(content.clone(), vec![1; 8]);
    let mut fetcher = HttpFetcher { port: peer.port };
    let mut ledger = MemoryLedger::default();

    let error = download_package(
        &op(),
        &fixture.request(
            FetchPolicy {
                max_attempts_per_window: 3,
            },
            VerifyLimits::default(),
        ),
        &mut fetcher,
        &mut ledger,
    )
    .expect_err("a source that never completes must stop by name, not loop");
    assert_eq!(error.code, ErrorCode::ResourceExhausted);
    assert_eq!(error.reason, "download retry budget exhausted");
    // One byte became durable per attempt, so each request resumed one byte
    // further along: the budget bounds the attempts *and* the progress is real.
    assert_eq!(
        *peer.ranges.lock().expect("the recorder lock"),
        vec![0u64, 1u64, 2u64]
    );
    assert_eq!(ledger.durable_bytes(ARTIFACT), 3);
}
