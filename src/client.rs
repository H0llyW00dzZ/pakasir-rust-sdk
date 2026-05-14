// Copyright 2026 H0llyW00dzZ
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! HTTP transport for the SDK.
//!
//! [`Client`] holds the merchant credentials, the underlying
//! [`reqwest::Client`], and the policy knobs (retries, backoff, response size
//! limit, language). Service types in [`crate::transaction`] and
//! [`crate::simulation`] wrap a `Client` and call [`Client::do_request`].
//!
//! Retry behavior:
//!
//! - `429`, `502`, `503`, `504` are retried.
//! - `500` is not retried; it usually means a server bug, not a transient
//!   condition.
//! - Transport errors are retried unless they come from invalid builder
//!   configuration.
//! - `Retry-After` (seconds or HTTP-date) is honored and capped at
//!   [`DEFAULT_RETRY_WAIT_MAX`] (or whatever the builder sets).
//!
//! Use [`Client::builder`] to configure a client.

use rand::random;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderValue, RETRY_AFTER};
use reqwest::{Method, Response, Url};
use std::time::Duration;

use crate::constants::user_agent;
use crate::error::{BoxError, Error, Result};
use crate::i18n::Language;
#[cfg(feature = "qr")]
use crate::qr::{Options as QrOptions, QrGenerator};

/// Production base URL.
pub const DEFAULT_BASE_URL: &str = "https://app.pakasir.com";
/// Per-request timeout used when the SDK builds its own [`reqwest::Client`].
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// Number of additional attempts after the first one fails.
pub const DEFAULT_RETRIES: usize = 3;
/// Minimum sleep between retries.
pub const DEFAULT_RETRY_WAIT_MIN: Duration = Duration::from_secs(1);
/// Maximum sleep between retries. Also the ceiling for any `Retry-After`
/// hint coming from the server.
pub const DEFAULT_RETRY_WAIT_MAX: Duration = Duration::from_secs(30);
/// Cap on a single response body, in bytes (1 MiB).
///
/// Anything larger is rejected with [`Error::ResponseTooLarge`] before being
/// buffered.
pub const DEFAULT_MAX_RESPONSE_SIZE: usize = 1 << 20;

/// Async HTTP client for the Pakasir REST API.
///
/// Cloning a `Client` is cheap (the inner [`reqwest::Client`] is reference
/// counted) and safe across tasks. Build one with [`Client::new`] or
/// [`Client::builder`] and pass it into the service types.
#[derive(Debug, Clone)]
pub struct Client {
    project: String,
    api_key: String,
    base_url: String,
    http_client: reqwest::Client,
    language: Language,
    retries: usize,
    retry_wait_min: Duration,
    retry_wait_max: Duration,
    max_response_size: usize,
    #[cfg(feature = "qr")]
    qr: QrGenerator,
}

/// Builder for [`Client`].
///
/// Returned by [`Client::builder`]. Setters consume and return `self`; finish
/// with [`ClientBuilder::build`].
#[derive(Debug, Clone)]
pub struct ClientBuilder {
    project: String,
    api_key: String,
    base_url: String,
    http_client: Option<reqwest::Client>,
    timeout: Duration,
    language: Language,
    retries: usize,
    retry_wait_min: Duration,
    retry_wait_max: Duration,
    max_response_size: usize,
    #[cfg(feature = "qr")]
    qr_options: QrOptions,
}

/// Outcome of a single request attempt.
///
/// `Stop` means we are done and the caller should see this error.
/// `Retry` carries the underlying error and an optional `Retry-After` hint
/// so the loop can wait the right amount before trying again.
enum AttemptError {
    Stop(Error),
    Retry {
        source: BoxError,
        retry_after_hint: Option<Duration>,
    },
}

impl Client {
    /// Build a client with default settings.
    ///
    /// Same as `Client::builder(project, api_key).build()`. Credential
    /// validation is deferred until the first [`Client::do_request`] call,
    /// so this never fails.
    pub fn new(project: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self::builder(project, api_key).build()
    }

    /// Start a [`ClientBuilder`].
    pub fn builder(project: impl Into<String>, api_key: impl Into<String>) -> ClientBuilder {
        ClientBuilder::new(project, api_key)
    }

    /// Configured project slug.
    pub fn project(&self) -> &str {
        &self.project
    }

    /// Configured API key.
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Language used when formatting localized error messages.
    pub fn language(&self) -> Language {
        self.language
    }

    /// Borrow the QR generator attached to this client.
    ///
    /// Available only when the `qr` feature is enabled.
    #[cfg(feature = "qr")]
    pub fn qr(&self) -> &QrGenerator {
        &self.qr
    }

    /// Send a request to the API.
    ///
    /// This is the low-level entry point used by the service modules. It
    /// validates credentials, builds the URL from `base_url + path`, applies
    /// retry / backoff / `Retry-After`, enforces the response size limit,
    /// and turns non-success responses into [`Error::Api`].
    ///
    /// The returned bytes are the raw response body; JSON decoding is left
    /// to the caller.
    pub async fn do_request(
        &self,
        method: Method,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<Vec<u8>> {
        self.validate_credentials()?;

        let mut last_error: Option<BoxError> = None;
        let mut retry_after_hint = None;

        for attempt in 0..=self.retries {
            self.wait_for_retry(attempt, retry_after_hint).await;

            match self
                .execute_attempt(method.clone(), path, body.as_deref())
                .await
            {
                Ok(bytes) => return Ok(bytes),
                Err(AttemptError::Stop(error)) => return Err(error),
                Err(AttemptError::Retry {
                    source,
                    retry_after_hint: hint,
                }) => {
                    last_error = Some(source);
                    retry_after_hint = hint;
                }
            }
        }

        let source: BoxError = last_error
            .unwrap_or_else(|| Box::new(std::io::Error::other("request failed")) as BoxError);
        Err(Error::request_failed_after_retries(
            self.language,
            self.retries,
            source,
        ))
    }

    /// Reject empty credentials before any network call is made.
    fn validate_credentials(&self) -> Result<()> {
        if self.project.is_empty() {
            return Err(Error::invalid_project(self.language));
        }
        if self.api_key.is_empty() {
            return Err(Error::invalid_api_key(self.language));
        }
        Ok(())
    }

    /// Run one HTTP attempt and classify the result.
    ///
    /// Errors come back as [`AttemptError::Stop`] (do not retry) or
    /// [`AttemptError::Retry`] (retry, optionally honoring a `Retry-After`
    /// hint from the response).
    async fn execute_attempt(
        &self,
        method: Method,
        path: &str,
        body: Option<&[u8]>,
    ) -> std::result::Result<Vec<u8>, AttemptError> {
        let url = self.build_url(path).map_err(AttemptError::Stop)?;

        let mut request = self
            .http_client
            .request(method, url)
            .header(ACCEPT, HeaderValue::from_static("application/json"));

        if let Some(body) = body {
            request = request
                .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                .body(body.to_vec());
        }

        let response = request.send().await.map_err(|err| {
            if is_retryable_transport(&err) {
                AttemptError::Retry {
                    source: Box::new(err),
                    retry_after_hint: None,
                }
            } else {
                AttemptError::Stop(Error::request_failed(self.language, Box::new(err)))
            }
        })?;

        self.handle_response(response).await
    }

    /// Read the body (subject to [`Client::max_response_size`]), look at the
    /// status, and decide between success, permanent failure, and retry.
    async fn handle_response(
        &self,
        response: Response,
    ) -> std::result::Result<Vec<u8>, AttemptError> {
        let status = response.status();
        let retry_after_hint = parse_retry_after(response.headers().get(RETRY_AFTER));

        let body = self
            .read_response_body(response)
            .await
            .map_err(|err| match err {
                Error::ResponseTooLarge { .. } => {
                    AttemptError::Stop(Error::request_failed(self.language, Box::new(err)))
                }
                other => AttemptError::Retry {
                    source: Box::new(other),
                    retry_after_hint: None,
                },
            })?;

        if status.is_success() {
            return Ok(body);
        }

        let api_error = Error::Api {
            status,
            body: String::from_utf8_lossy(&body).into_owned(),
        };

        if is_retryable_status(status) {
            return Err(AttemptError::Retry {
                source: Box::new(api_error),
                retry_after_hint,
            });
        }

        Err(AttemptError::Stop(api_error))
    }

    /// Read the response body in chunks. Bail out with
    /// [`Error::ResponseTooLarge`] as soon as the running total exceeds
    /// [`Client::max_response_size`], so an oversized payload is never fully
    /// buffered.
    async fn read_response_body(&self, mut response: Response) -> Result<Vec<u8>> {
        let mut body = Vec::new();

        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|err| Error::request_failed(self.language, Box::new(err)))?
        {
            body.extend_from_slice(&chunk);
            if body.len() > self.max_response_size {
                return Err(Error::ResponseTooLarge {
                    limit: self.max_response_size,
                });
            }
        }

        Ok(body)
    }

    /// Sleep before the next attempt.
    ///
    /// The first iteration (`attempt == 0`) returns immediately. After that,
    /// a `Retry-After` hint wins (clamped to [`Client::retry_wait_max`]) and
    /// otherwise we fall back to [`Client::calculate_backoff`].
    async fn wait_for_retry(&self, attempt: usize, retry_after_hint: Option<Duration>) {
        if attempt == 0 {
            return;
        }

        let wait = retry_after_hint
            .map(|hint| hint.min(self.retry_wait_max))
            .unwrap_or_else(|| self.calculate_backoff(attempt));

        tokio::time::sleep(wait).await;
    }

    /// Jittered exponential backoff bounded by
    /// `[retry_wait_min, retry_wait_max]`.
    ///
    /// The multiplier doubles each attempt (`1, 2, 4, …`) and the resulting
    /// window is randomized so concurrent callers don't retry in lockstep.
    fn calculate_backoff(&self, attempt: usize) -> Duration {
        let multiplier = 1u32
            .checked_shl((attempt.saturating_sub(1)) as u32)
            .unwrap_or(u32::MAX);
        let max_wait = self
            .retry_wait_min
            .saturating_mul(multiplier)
            .min(self.retry_wait_max);

        if max_wait <= self.retry_wait_min {
            return self.retry_wait_min;
        }

        let span_nanos = max_wait
            .saturating_sub(self.retry_wait_min)
            .as_nanos()
            .min(u64::MAX as u128) as u64;
        let jitter = random::<u64>() % (span_nanos + 1);
        self.retry_wait_min + Duration::from_nanos(jitter)
    }

    /// Join the base URL and `path` and parse the result.
    ///
    /// Returns [`Error::BuildRequest`] on a parse failure.
    fn build_url(&self, path: &str) -> Result<Url> {
        Url::parse(&format!("{}{}", self.base_url, path))
            .map_err(|source| Error::BuildRequest { source })
    }
}

impl ClientBuilder {
    /// New builder with all defaults applied. Most callers should use
    /// [`Client::builder`] instead.
    pub fn new(project: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            project: project.into(),
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_owned(),
            http_client: None,
            timeout: DEFAULT_TIMEOUT,
            language: Language::English,
            retries: DEFAULT_RETRIES,
            retry_wait_min: DEFAULT_RETRY_WAIT_MIN,
            retry_wait_max: DEFAULT_RETRY_WAIT_MAX,
            max_response_size: DEFAULT_MAX_RESPONSE_SIZE,
            #[cfg(feature = "qr")]
            qr_options: QrOptions::default(),
        }
    }

    /// Override the API base URL. Trailing slashes are stripped so endpoint
    /// paths never end up with `//` in them.
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_owned();
        self
    }

    /// Use a custom [`reqwest::Client`] (shared pool, proxy, custom TLS, …).
    /// When set, the [`ClientBuilder::timeout`] value is ignored — configure
    /// the timeout on the client you pass in.
    pub fn http_client(mut self, http_client: reqwest::Client) -> Self {
        self.http_client = Some(http_client);
        self
    }

    /// Per-request timeout for the SDK's default HTTP client. Zero is
    /// ignored so the default is kept.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        if !timeout.is_zero() {
            self.timeout = timeout;
        }
        self
    }

    /// Language used for localized error messages.
    pub fn language(mut self, language: Language) -> Self {
        self.language = language;
        self
    }

    /// Number of retry attempts after the first one. `0` disables retries.
    pub fn retries(mut self, retries: usize) -> Self {
        self.retries = retries;
        self
    }

    /// Set the backoff bounds.
    ///
    /// Zero durations are clamped to 1 ms. If `min > max` the two are
    /// swapped so the resulting interval is always sane.
    pub fn retry_wait(mut self, min: Duration, max: Duration) -> Self {
        let floor = Duration::from_millis(1);
        let mut resolved_min = if min.is_zero() { floor } else { min };
        let mut resolved_max = if max.is_zero() { floor } else { max };

        if resolved_min > resolved_max {
            std::mem::swap(&mut resolved_min, &mut resolved_max);
        }

        self.retry_wait_min = resolved_min;
        self.retry_wait_max = resolved_max;
        self
    }

    /// Maximum response body size in bytes. Zero is ignored.
    pub fn max_response_size(mut self, max_response_size: usize) -> Self {
        if max_response_size > 0 {
            self.max_response_size = max_response_size;
        }
        self
    }

    /// QR generator settings exposed through [`Client::qr`].
    ///
    /// Available only when the `qr` feature is enabled.
    #[cfg(feature = "qr")]
    pub fn qr_options(mut self, qr_options: QrOptions) -> Self {
        self.qr_options = qr_options;
        self
    }

    /// Finalize the builder.
    ///
    /// If no custom [`reqwest::Client`] was supplied, the default one is
    /// built with the configured timeout and the SDK user-agent. A failure
    /// at this point would be a bug in the library, so it panics.
    pub fn build(self) -> Client {
        let http_client = self.http_client.unwrap_or_else(|| {
            reqwest::Client::builder()
                .timeout(self.timeout)
                .user_agent(user_agent())
                .build()
                .expect("default reqwest client configuration must be valid")
        });

        Client {
            project: self.project,
            api_key: self.api_key,
            base_url: self.base_url,
            http_client,
            language: self.language,
            retries: self.retries,
            retry_wait_min: self.retry_wait_min,
            retry_wait_max: self.retry_wait_max,
            max_response_size: self.max_response_size,
            #[cfg(feature = "qr")]
            qr: QrGenerator::new(self.qr_options),
        }
    }
}

/// HTTP statuses the SDK treats as transient: `429`, `502`, `503`, `504`.
///
/// `500` is left out on purpose. It usually means a deterministic server
/// bug, not something a retry will fix.
fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    matches!(
        status,
        reqwest::StatusCode::TOO_MANY_REQUESTS
            | reqwest::StatusCode::BAD_GATEWAY
            | reqwest::StatusCode::SERVICE_UNAVAILABLE
            | reqwest::StatusCode::GATEWAY_TIMEOUT
    )
}

/// Transport errors worth retrying.
///
/// Builder errors mean the request was never going to be valid in the first
/// place, so we don't retry those. Anything else (connect, TLS, body
/// stream, …) is treated as transient.
fn is_retryable_transport(error: &reqwest::Error) -> bool {
    !error.is_builder()
}

/// Parse a `Retry-After` header into a [`Duration`].
///
/// Both forms from RFC 7231 are supported:
///
/// - **delta-seconds** – integer seconds, capped at 24h to keep the
///   resulting [`Duration`] in range.
/// - **HTTP-date** – parsed with [`httpdate`] and converted to a duration
///   relative to [`std::time::SystemTime::now`].
///
/// Returns `None` when the header is missing, empty, or unparseable.
fn parse_retry_after(value: Option<&HeaderValue>) -> Option<Duration> {
    let raw = value?.to_str().ok()?.trim();
    if raw.is_empty() {
        return None;
    }

    if let Ok(seconds) = raw.parse::<u64>() {
        return Some(Duration::from_secs(seconds.min(86_400)));
    }

    let parsed = httpdate::parse_http_date(raw).ok()?;
    parsed.duration_since(std::time::SystemTime::now()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    #[test]
    fn client_new_yields_same_defaults_as_builder() {
        let a = Client::new("p", "k");
        let b = Client::builder("p", "k").build();
        assert_eq!(a.project(), b.project());
        assert_eq!(a.api_key(), b.api_key());
        assert_eq!(a.language(), b.language());
        assert_eq!(a.retries, b.retries);
        assert_eq!(a.retry_wait_min, b.retry_wait_min);
        assert_eq!(a.retry_wait_max, b.retry_wait_max);
        assert_eq!(a.max_response_size, b.max_response_size);
        assert_eq!(a.base_url, b.base_url);
    }

    #[test]
    fn client_getters_return_configured_values() {
        let client = Client::builder("proj", "key").build();
        assert_eq!(client.project(), "proj");
        assert_eq!(client.api_key(), "key");
        assert_eq!(client.language(), Language::English);
    }

    #[test]
    fn builder_base_url_strips_trailing_slashes() {
        let client = Client::builder("p", "k").base_url("https://x/").build();
        assert_eq!(client.base_url, "https://x");

        let client = Client::builder("p", "k").base_url("https://x///").build();
        assert_eq!(client.base_url, "https://x");
    }

    #[test]
    fn builder_http_client_swaps_underlying_reqwest_client() {
        let custom = reqwest::Client::builder().build().unwrap();
        // No public way to identify the inner client, but build() must not
        // panic and the resulting Client must be usable.
        let _ = Client::builder("p", "k").http_client(custom).build();
    }

    #[test]
    fn builder_timeout_zero_is_a_no_op() {
        let client = Client::builder("p", "k").timeout(Duration::ZERO).build();
        // The Client doesn't expose its timeout; we just verify the builder
        // chain executes the no-op branch without panicking.
        let _ = client;
    }

    #[test]
    fn builder_timeout_applies_positive_durations() {
        let _client = Client::builder("p", "k")
            .timeout(Duration::from_secs(7))
            .build();
    }

    #[test]
    fn builder_language_overrides_default() {
        let client = Client::builder("p", "k")
            .language(Language::Indonesian)
            .build();
        assert_eq!(client.language(), Language::Indonesian);
    }

    #[test]
    fn builder_retries_overrides_default() {
        let client = Client::builder("p", "k").retries(7).build();
        assert_eq!(client.retries, 7);

        let client = Client::builder("p", "k").retries(0).build();
        assert_eq!(client.retries, 0);
    }

    #[test]
    fn builder_retry_wait_zero_durations_clamp_to_one_millisecond() {
        let client = Client::builder("p", "k")
            .retry_wait(Duration::ZERO, Duration::ZERO)
            .build();
        assert_eq!(client.retry_wait_min, Duration::from_millis(1));
        assert_eq!(client.retry_wait_max, Duration::from_millis(1));
    }

    #[test]
    fn builder_retry_wait_swaps_min_and_max_when_inverted() {
        let client = Client::builder("p", "k")
            .retry_wait(Duration::from_secs(10), Duration::from_secs(1))
            .build();
        // 10s > 1s → should be swapped so min < max.
        assert_eq!(client.retry_wait_min, Duration::from_secs(1));
        assert_eq!(client.retry_wait_max, Duration::from_secs(10));
    }

    #[test]
    fn builder_retry_wait_keeps_already_ordered_pair() {
        let client = Client::builder("p", "k")
            .retry_wait(Duration::from_millis(100), Duration::from_millis(500))
            .build();
        assert_eq!(client.retry_wait_min, Duration::from_millis(100));
        assert_eq!(client.retry_wait_max, Duration::from_millis(500));
    }

    #[test]
    fn builder_max_response_size_zero_is_a_no_op() {
        let client = Client::builder("p", "k").max_response_size(0).build();
        assert_eq!(client.max_response_size, DEFAULT_MAX_RESPONSE_SIZE);
    }

    #[test]
    fn builder_max_response_size_overrides_default() {
        let client = Client::builder("p", "k").max_response_size(512).build();
        assert_eq!(client.max_response_size, 512);
    }

    #[cfg(feature = "qr")]
    #[test]
    fn builder_qr_options_propagate_to_client() {
        use crate::qr::{Options as QrOpts, RecoveryLevel};
        let opts = QrOpts::default()
            .with_size(384)
            .with_recovery_level(RecoveryLevel::High);
        let client = Client::builder("p", "k").qr_options(opts.clone()).build();
        assert_eq!(client.qr().options(), &opts);
    }

    #[test]
    fn is_retryable_status_matches_documented_set() {
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_status(StatusCode::BAD_GATEWAY));
        assert!(is_retryable_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_retryable_status(StatusCode::GATEWAY_TIMEOUT));
    }

    #[test]
    fn is_retryable_status_excludes_other_statuses() {
        for code in [
            StatusCode::OK,
            StatusCode::BAD_REQUEST,
            StatusCode::NOT_FOUND,
            StatusCode::UNAUTHORIZED,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::NOT_IMPLEMENTED,
        ] {
            assert!(!is_retryable_status(code), "must NOT retry on {code}");
        }
    }

    #[test]
    fn parse_retry_after_returns_none_for_missing_header() {
        assert!(parse_retry_after(None).is_none());
    }

    #[test]
    fn parse_retry_after_returns_none_for_empty_value() {
        let header = HeaderValue::from_static("");
        assert!(parse_retry_after(Some(&header)).is_none());

        let header = HeaderValue::from_static("   ");
        assert!(parse_retry_after(Some(&header)).is_none());
    }

    #[test]
    fn parse_retry_after_returns_none_for_unparseable_value() {
        let header = HeaderValue::from_static("not a real value");
        assert!(parse_retry_after(Some(&header)).is_none());
    }

    #[test]
    fn parse_retry_after_parses_delta_seconds() {
        let header = HeaderValue::from_static("12");
        assert_eq!(
            parse_retry_after(Some(&header)),
            Some(Duration::from_secs(12))
        );
    }

    #[test]
    fn parse_retry_after_caps_delta_seconds_at_24h() {
        let header = HeaderValue::from_static("999999"); // way more than 24h
        assert_eq!(
            parse_retry_after(Some(&header)),
            Some(Duration::from_secs(86_400))
        );
    }

    #[test]
    fn parse_retry_after_parses_http_date_in_the_future() {
        // Build a date a long way in the future so the duration is positive
        // regardless of clock skew.
        let target = std::time::SystemTime::now() + Duration::from_secs(60);
        let formatted = httpdate::fmt_http_date(target);
        let header = HeaderValue::from_str(&formatted).unwrap();
        let parsed = parse_retry_after(Some(&header)).expect("future HTTP-date should parse");
        // HTTP-date only carries second precision, so allow a small window.
        assert!(parsed <= Duration::from_secs(61));
    }

    #[test]
    fn parse_retry_after_returns_none_for_http_date_in_the_past() {
        let header = HeaderValue::from_static("Wed, 21 Oct 1970 07:28:00 GMT");
        assert!(parse_retry_after(Some(&header)).is_none());
    }

    #[test]
    fn calculate_backoff_floors_at_retry_wait_min() {
        let client = Client::builder("p", "k")
            .retry_wait(Duration::from_millis(10), Duration::from_millis(20))
            .build();
        // attempt = 0 → max_wait == retry_wait_min → return min directly
        assert_eq!(client.calculate_backoff(0), Duration::from_millis(10));
    }

    #[test]
    fn calculate_backoff_stays_within_configured_bounds() {
        let client = Client::builder("p", "k")
            .retry_wait(Duration::from_millis(10), Duration::from_millis(20))
            .build();
        for attempt in 1..=5_usize {
            let wait = client.calculate_backoff(attempt);
            assert!(
                wait >= Duration::from_millis(10),
                "attempt={attempt} wait={wait:?}"
            );
            assert!(
                wait <= Duration::from_millis(20),
                "attempt={attempt} wait={wait:?}"
            );
        }
    }

    #[test]
    fn calculate_backoff_handles_extreme_attempt_count() {
        let client = Client::builder("p", "k")
            .retry_wait(Duration::from_millis(10), Duration::from_secs(30))
            .build();
        // Guard against shift overflow on huge attempt counts.
        let wait = client.calculate_backoff(64);
        assert!(wait <= Duration::from_secs(30));
    }

    #[tokio::test]
    async fn do_request_rejects_empty_project() {
        let client = Client::builder("", "k").retries(0).build();
        let err = client
            .do_request(Method::GET, "/x", None)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::InvalidProject { .. }));
    }

    #[tokio::test]
    async fn do_request_rejects_empty_api_key() {
        let client = Client::builder("p", "").retries(0).build();
        let err = client
            .do_request(Method::GET, "/x", None)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::InvalidApiKey { .. }));
    }

    #[tokio::test]
    async fn do_request_surfaces_build_url_failures() {
        // Configure a base URL that does not parse as a URL on its own.
        // `Client::do_request` glues `base_url + path` and parses the result.
        let client = Client::builder("p", "k")
            .base_url("not a url")
            .retries(0)
            .build();
        let err = client
            .do_request(Method::GET, "/path", None)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::BuildRequest { .. }));
    }

    #[tokio::test]
    async fn wait_for_retry_returns_immediately_on_first_attempt() {
        let client = Client::builder("p", "k")
            .retry_wait(Duration::from_secs(60), Duration::from_secs(60))
            .build();
        // Even with a huge retry_wait, attempt == 0 must not sleep.
        let start = std::time::Instant::now();
        client.wait_for_retry(0, None).await;
        assert!(start.elapsed() < Duration::from_millis(500));
    }

    #[tokio::test]
    async fn wait_for_retry_honors_retry_after_hint_clamped_to_max() {
        let client = Client::builder("p", "k")
            .retry_wait(Duration::from_millis(1), Duration::from_millis(5))
            .build();
        // Hint larger than retry_wait_max gets clamped, so this sleeps ~5ms.
        let start = std::time::Instant::now();
        client
            .wait_for_retry(1, Some(Duration::from_secs(60)))
            .await;
        assert!(start.elapsed() < Duration::from_millis(500));
    }
}
