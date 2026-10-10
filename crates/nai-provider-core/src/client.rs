use nai_contracts::v1::ErrorCode;
use nai_security::{Operation, Secret};
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Provider transport error mapped to contract codes. Retryable errors are
/// explicit so callers never mistake an auth failure for a transient outage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderError {
    pub code: ErrorCode,
    pub retryable: bool,
    pub message: String,
}
impl ProviderError {
    pub(crate) fn new(code: ErrorCode, retryable: bool, message: impl Into<String>) -> Self {
        Self {
            code,
            retryable,
            message: message.into(),
        }
    }
    pub fn cancelled() -> Self {
        Self::new(ErrorCode::Cancelled, false, "request cancelled")
    }
    pub fn deadline() -> Self {
        Self::new(
            ErrorCode::DeadlineExceeded,
            false,
            "request deadline exceeded",
        )
    }
    /// A provider that could not be reached or answered in time. Retryable.
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ProviderUnavailable, true, message)
    }
    /// A caller-supplied argument the provider layer refuses. Not retryable.
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidArgument, false, message)
    }
    fn exhausted(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ResourceExhausted, false, message)
    }
    fn auth(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::new(code, false, message)
    }
}
impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "provider {} ({})", self.code.as_str_name(), self.message)
    }
}
impl std::error::Error for ProviderError {}

/// How a binding presents its credential on the wire. The scheme belongs to
/// the provider configuration, never to the shared request path: adding a
/// provider must not mean editing transport code. `Bearer` stays the default so
/// **not** declaring a scheme is an explicit OpenAI-compatible choice rather
/// than a silent one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AuthScheme {
    /// `Authorization: Bearer <credential>`.
    #[default]
    Bearer,
    /// A provider-specific credential header (e.g. `x-api-key`, `xi-api-key`).
    /// The value is the configured credential, exactly as `ApiKey` is for
    /// `Bearer`; no prefix, encoding or transformation is applied.
    Header { name: String },
}

/// Header names the transport owns. A binding may not override them: a
/// provider that needs its own `Authorization`/`Host`/body framing is a
/// different transport, not a header.
const MANAGED_HEADERS: [&str; 4] = ["authorization", "host", "content-length", "content-type"];
/// Bounds on binding-supplied static headers.
const MAX_EXTRA_HEADERS: usize = 8;
const MAX_HEADER_NAME: usize = 64;
const MAX_HEADER_VALUE: usize = 1024;

/// A single header name is an RFC 9110 token, compared case-insensitively.
fn valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_HEADER_NAME
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
        && !MANAGED_HEADERS.contains(&name.to_ascii_lowercase().as_str())
}

pub struct HttpConfig {
    /// Stable identity of the provider this transport talks to. Recorded in
    /// usage charges and checked against the binding, so a profile swap is
    /// visible instead of two configs silently sharing one ledger key.
    pub provider_ref: String,
    /// Base URL, e.g. `https://api.openai.com/v1`. Transport never logs it.
    pub endpoint: String,
    /// Credential supplied by the composition root; never logged or stored.
    /// Its presentation is decided by `auth`, not by this field.
    pub api_key: Option<Secret>,
    /// Credential presentation for this provider.
    pub auth: AuthScheme,
    /// Static headers the provider requires (e.g. `anthropic-version`). Never
    /// carries the credential; that is `api_key` + `auth`.
    pub extra_headers: Vec<(String, String)>,
    /// Model name from the trusted ProviderConfig profile.
    pub model: String,
    /// Exact price scope passed through to BudgetCharge.
    pub price_revision: String,
    pub currency: String,
    /// Task deadline cap applied per attempt; never exceeds the Operation.
    pub timeout: Duration,
    /// Extra attempts after the first (0 = no retries). 401/403 never retry.
    pub max_retries: u32,
}

/// A successful 2xx response ready for json/stream consumption.
pub struct StreamResponse {
    pub status: u16,
    pub body: reqwest::Response,
}

pub struct ProviderClient {
    http: reqwest::Client,
    config: HttpConfig,
}
impl ProviderClient {
    pub fn new(config: HttpConfig) -> Result<Self, ProviderError> {
        if config.endpoint.is_empty()
            || config.endpoint.len() > 2048
            || config.endpoint.starts_with("://")
            || config.timeout.is_zero()
            || config.timeout > Duration::from_secs(600)
            || config.max_retries > 10
            || config.model.is_empty()
            || config.model.len() > 256
            || config.price_revision.is_empty()
            || config.currency.len() != 3
            || !config.currency.bytes().all(|b| b.is_ascii_uppercase())
        {
            return Err(ProviderError::new(
                ErrorCode::InvalidArgument,
                false,
                "invalid provider transport config",
            ));
        }
        if config.provider_ref.is_empty() || config.provider_ref.len() > 128 {
            return Err(ProviderError::new(
                ErrorCode::InvalidArgument,
                false,
                "provider binding has no usable identity",
            ));
        }
        // A credential header the transport cannot own is refused up front, so
        // no request is ever built with an unusable credential placement.
        if matches!(&config.auth, AuthScheme::Header { name } if !valid_header_name(name)) {
            return Err(ProviderError::new(
                ErrorCode::InvalidArgument,
                false,
                "provider credential header rejected",
            ));
        }
        if config.extra_headers.len() > MAX_EXTRA_HEADERS {
            return Err(ProviderError::new(
                ErrorCode::InvalidArgument,
                false,
                "too many provider headers",
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for (name, value) in &config.extra_headers {
            // A header the binding may not own, a duplicate, a folded value or
            // a response-splitting payload is refused before any request is
            // built, not sanitised afterwards.
            if !valid_header_name(name)
                || !seen.insert(name.to_ascii_lowercase())
                || value.is_empty()
                || value.len() > MAX_HEADER_VALUE
                || value.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0)
            {
                return Err(ProviderError::new(
                    ErrorCode::InvalidArgument,
                    false,
                    "provider header rejected",
                ));
            }
        }
        let http = reqwest::Client::builder()
            .connect_timeout(config.timeout)
            .build()
            .map_err(|error| ProviderError::unavailable(format!("client build: {error}")))?;
        Ok(Self { http, config })
    }
    pub fn config(&self) -> &HttpConfig {
        &self.config
    }
    fn check(&self, op: &Operation) -> Result<(), ProviderError> {
        op.check().map_err(|error| match error.code {
            ErrorCode::Cancelled => ProviderError::cancelled(),
            ErrorCode::DeadlineExceeded => ProviderError::deadline(),
            _ => ProviderError::new(ErrorCode::Internal, false, "operation failed"),
        })
    }
    fn credential(&self) -> Result<&str, ProviderError> {
        self.config
            .api_key
            .as_ref()
            .map(|secret| {
                std::str::from_utf8(secret.as_bytes())
                    .map_err(|_| ProviderError::auth(ErrorCode::Unauthenticated, "invalid secret"))
            })
            .transpose()?
            .ok_or_else(|| {
                ProviderError::new(
                    ErrorCode::Unauthenticated,
                    false,
                    "provider credential unavailable",
                )
            })
    }
    fn request(
        &self,
        op: &Operation,
        method: &str,
        path: &str,
        body: &Value,
    ) -> Result<reqwest::RequestBuilder, ProviderError> {
        if !path.starts_with('/') || body.as_object().is_none() {
            return Err(ProviderError::new(
                ErrorCode::InvalidArgument,
                false,
                "invalid provider request",
            ));
        }
        let url = format!("{}{}", self.config.endpoint.trim_end_matches('/'), path);
        let credential = self.credential()?;
        let mut request = self
            .http
            .request(
                reqwest::Method::from_bytes(method.as_bytes()).map_err(|_| {
                    ProviderError::new(ErrorCode::InvalidArgument, false, "invalid method")
                })?,
                url,
            )
            // The credential presentation is a property of the binding, so the
            // shared request path is identical for every provider.
            .header(
                match &self.config.auth {
                    AuthScheme::Bearer => reqwest::header::AUTHORIZATION,
                    AuthScheme::Header { name } => {
                        reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
                            ProviderError::new(ErrorCode::InvalidArgument, false, "invalid header")
                        })?
                    }
                },
                match &self.config.auth {
                    AuthScheme::Bearer => format!("Bearer {credential}"),
                    AuthScheme::Header { .. } => credential.to_string(),
                },
            );
        for (name, value) in &self.config.extra_headers {
            request = request.header(name.as_str(), value.as_str());
        }
        Ok(request
            .json(body)
            .timeout(op.remaining().min(self.config.timeout)))
    }
    /// Reads a bounded provider error body, preferring a JSON `error.message`.
    async fn provider_message(&self, response: reqwest::Response) -> String {
        let bytes = match tokio::time::timeout(Duration::from_secs(2), response.bytes()).await {
            Ok(Ok(bytes)) if bytes.len() <= 8 * 1024 => bytes,
            _ => return "unavailable".to_string(),
        };
        serde_json::from_slice::<Value>(&bytes)
            .ok()
            .map(|json| {
                json.pointer("/error/message")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        serde_json::to_string(&json).unwrap_or_else(|_| "unavailable".to_string())
                    })
            })
            .unwrap_or_else(|| String::from_utf8_lossy(&bytes).into_owned())
    }
    async fn sleep_until(&self, op: &Operation, target: Instant) -> Result<(), ProviderError> {
        loop {
            self.check(op)?;
            let now = Instant::now();
            if now >= target {
                return Ok(());
            }
            sleep(Duration::from_millis(50).min(target - now)).await;
        }
    }
    /// Executes a request with bounded retry. On 2xx returns the raw response;
    /// 401/403 never retry; 429 and 5xx retry only within the task deadline.
    pub async fn send(
        &self,
        op: &Operation,
        method: &str,
        path: &str,
        body: &Value,
    ) -> Result<reqwest::Response, ProviderError> {
        let mut attempt = 0u32;
        loop {
            self.check(op)?;
            let request = self.request(op, method, path, body)?;
            let response = request
                .send()
                .await
                .map_err(|error| {
                    if error.is_timeout() {
                        if op.stopped() {
                            ProviderError::deadline()
                        } else {
                            ProviderError::unavailable("provider timed out")
                        }
                    } else if error.is_connect() {
                        ProviderError::unavailable("provider connection failed")
                    } else {
                        ProviderError::unavailable(format!("provider transport: {error}"))
                    }
                })
                .map(|response| (response.status().as_u16(), response))?;
            let status = response.0;
            if (200..300).contains(&status) {
                return Ok(response.1);
            }
            let retry_after = response
                .1
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            if attempt >= self.config.max_retries {
                let detail = self.provider_message(response.1).await;
                return Err(match status {
                    401 => ProviderError::auth(
                        ErrorCode::Unauthenticated,
                        format!("provider rejected credentials: {detail}"),
                    ),
                    403 => ProviderError::auth(
                        ErrorCode::PermissionDenied,
                        format!("provider denied request: {detail}"),
                    ),
                    429 => ProviderError::exhausted(format!("provider rate limited: {detail}")),
                    _ => ProviderError::unavailable(format!("provider http {status}: {detail}")),
                });
            }
            let backoff = match status {
                401 | 403 => {
                    return Err(ProviderError::auth(
                        if status == 401 {
                            ErrorCode::Unauthenticated
                        } else {
                            ErrorCode::PermissionDenied
                        },
                        "provider rejected request",
                    ));
                }
                429 => Duration::from_secs(retry_after),
                _ => Duration::from_millis(200)
                    .checked_mul(1u32 << attempt.min(8))
                    .unwrap_or(Duration::from_secs(1)),
            };
            let now = Instant::now();
            let remaining = op.remaining();
            if backoff >= remaining {
                return Err(match status {
                    429 => ProviderError::exhausted("provider rate limited until deadline"),
                    _ => ProviderError::unavailable("provider unavailable within deadline"),
                });
            }
            let target = now + backoff;
            self.sleep_until(op, target).await?;
            attempt += 1;
        }
    }
    /// JSON request that collects the full body within the task deadline.
    pub async fn post_json(
        &self,
        op: &Operation,
        path: &str,
        body: &Value,
    ) -> Result<(u16, Value), ProviderError> {
        let response = self.send(op, "POST", path, body).await?;
        let status = response.status().as_u16();
        let remaining = op.remaining();
        let bytes = tokio::time::timeout(remaining, response.bytes())
            .await
            .map_err(|_| {
                if op.stopped() {
                    ProviderError::deadline()
                } else {
                    ProviderError::unavailable("provider body timed out")
                }
            })?
            .map_err(|error| ProviderError::unavailable(format!("provider body: {error}")))?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(ProviderError::new(
                ErrorCode::InvalidArgument,
                false,
                "provider response exceeds bound",
            ));
        }
        let json = serde_json::from_slice(&bytes).map_err(|error| {
            ProviderError::new(
                ErrorCode::Internal,
                false,
                format!("provider json: {error}"),
            )
        })?;
        Ok((status, json))
    }
    /// Streaming request: on 2xx returns the response body stream.
    pub async fn stream(
        &self,
        op: &Operation,
        path: &str,
        body: &Value,
    ) -> Result<StreamResponse, ProviderError> {
        let response = self.send(op, "POST", path, body).await?;
        Ok(StreamResponse {
            status: response.status().as_u16(),
            body: response,
        })
    }
}
