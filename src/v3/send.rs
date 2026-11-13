//! HTTP/JSON transport every Google Calendar coroutine delegates to.
//!
//! Builds the authorized request and parses the JSON response, or the
//! Calendar error envelope on failure.
//!
//! Calendar API reference: <https://developers.google.com/workspace/calendar/api/v3/reference>.

use core::{marker::PhantomData, time::Duration};

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use io_http::{
    coroutine::*,
    rfc3986::path::{HttpUrlBuildError, http_url_build},
    rfc6750::bearer::HttpAuthBearer,
    rfc9110::{
        request::HttpRequest,
        send::{HttpSendOutput, HttpSendYield},
    },
    rfc9112::send::{Http11Send, Http11SendError},
};
use log::{debug, trace};
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use serde_json::Value;
use thiserror::Error;
use url::Url;

use crate::coroutine::*;

/// Base URL of the Google Calendar REST API v3.
pub const GCAL_API_BASE: &str = "https://www.googleapis.com/calendar/v3/";

/// Unit marker deserialized from empty 2xx bodies (DELETE, clear,
/// transfer ownership, stop).
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct GcalNoResponse;

impl<'de> Deserialize<'de> for GcalNoResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let _ = serde::de::IgnoredAny::deserialize(deserializer)?;
        Ok(Self)
    }
}

/// Errors that can occur during a Google Calendar exchange.
#[derive(Debug, Error)]
pub enum GcalSendError {
    /// The underlying HTTP exchange failed.
    #[error("Calendar HTTP request failed: {0}")]
    Send(#[from] Http11SendError),
    /// The request body could not be serialized to JSON.
    #[error("Calendar request serialization failed: {0}")]
    SerializeRequest(#[source] serde_json::Error),
    /// The 2xx response body could not be parsed as JSON.
    #[error("Calendar response parsing failed: {0}")]
    ParseResponse(#[source] serde_json::Error),
    /// The request URL could not be built.
    #[error("Calendar URL parsing failed: {0}")]
    ParseUrl(#[from] url::ParseError),
    /// The request was rejected before being sent.
    #[error("Invalid Calendar request: {0}")]
    InvalidRequest(String),
    /// Calendar returned a non-2xx status with its error envelope.
    #[error("{0}")]
    Api(GcalApiError),
    /// The server answered with a redirect, which is never followed.
    #[error("Calendar server returned an unexpected redirect")]
    UnexpectedRedirect,
}

impl GcalSendError {
    /// Returns the API error when Calendar answered with a non-2xx
    /// status.
    pub fn api(&self) -> Option<&GcalApiError> {
        match self {
            Self::Api(err) => Some(err),
            _ => None,
        }
    }

    /// Returns the HTTP status code when the error is an API error.
    pub fn status(&self) -> Option<u16> {
        self.api().map(|err| err.status)
    }

    /// Returns how long the server asked to wait before a retry
    /// ([`GcalApiError::retry_after`]).
    pub fn retry_after(&self) -> Option<Duration> {
        self.api().and_then(|err| err.retry_after)
    }

    /// Whether the error is an API error worth retrying later
    /// ([`GcalApiError::is_retryable`]).
    pub fn is_retryable(&self) -> bool {
        self.api().is_some_and(GcalApiError::is_retryable)
    }

    /// Whether the error is a rate limit answer
    /// ([`GcalApiError::is_rate_limited`]).
    pub fn is_rate_limited(&self) -> bool {
        self.api().is_some_and(GcalApiError::is_rate_limited)
    }

    /// Whether the target of the request does not exist (404).
    pub fn is_not_found(&self) -> bool {
        self.status() == Some(404)
    }

    /// Whether the error is an expired or invalid sync token
    /// ([`GcalApiError::is_sync_token_expired`]), which callers recover
    /// from by re-baselining a full listing.
    pub fn is_sync_token_expired(&self) -> bool {
        self.api().is_some_and(GcalApiError::is_sync_token_expired)
    }

    /// Whether the error is a failed entity tag guard (412), meaning
    /// the resource moved since the etag the write carried was read.
    pub fn is_precondition_failed(&self) -> bool {
        matches!(self.status(), Some(412))
    }
}

/// The Calendar error envelope, read from a non-2xx answer.
///
/// Beside its code and message, the envelope lists reasons in
/// `errors[]` and `details[]` and a canonical `status`: they are kept
/// so callers match on codes rather than on the message text.
#[derive(Debug, Clone, Default, Eq, PartialEq, Error)]
#[error("Calendar API returned HTTP {status}: {message}")]
pub struct GcalApiError {
    /// The effective status code: `error.code` when present, the HTTP
    /// status otherwise.
    pub status: u16,
    /// The error message, from the envelope or the raw body.
    pub message: String,
    /// The reasons of `error.errors[]`, in order, such as `notFound`,
    /// `rateLimitExceeded`, `fullSyncRequired` or `conditionNotMet`.
    pub reasons: Vec<String>,
    /// The canonical status of `error.status`, such as `NOT_FOUND`,
    /// `PERMISSION_DENIED` or `RESOURCE_EXHAUSTED`.
    pub google_status: Option<String>,
    /// The reasons of `error.details[]` (`google.rpc.ErrorInfo`), such
    /// as `RATE_LIMIT_EXCEEDED`.
    pub detail_reasons: Vec<String>,
    /// How long the server asked to wait before a retry: the
    /// `Retry-After` header in its seconds form, else the delay of
    /// `error.details[]` (`google.rpc.RetryInfo`).
    pub retry_after: Option<Duration>,
    /// Whether the server allows reusing the TCP/TLS connection.
    pub keep_alive: bool,
}

impl GcalApiError {
    /// Reads the Calendar error envelope out of a non-2xx body, falling
    /// back to the raw body as message.
    ///
    /// The body tells neither the `Retry-After` header nor the
    /// connection reuse: the exchange fills those in.
    pub fn parse(http_status: u16, body: &[u8]) -> Self {
        let (status, message) = parse_api_error(http_status, body);
        let mut err = Self {
            status,
            message,
            ..Default::default()
        };

        if let Ok(envelope) = serde_json::from_slice::<ErrorEnvelope>(body) {
            let error = envelope.error;
            err.reasons = error.errors.into_iter().filter_map(|e| e.reason).collect();
            err.google_status = error.status;
            err.retry_after = error
                .details
                .iter()
                .find_map(|d| d.retry_delay.as_ref()?.as_str().and_then(parse_delay));
            err.detail_reasons = error.details.into_iter().filter_map(|d| d.reason).collect();
        }

        err
    }

    /// Whether `reason` is one of [`Self::reasons`] or
    /// [`Self::detail_reasons`].
    pub fn has_reason(&self, reason: &str) -> bool {
        self.reasons
            .iter()
            .chain(&self.detail_reasons)
            .any(|r| r == reason)
    }

    /// Whether Google throttled the request: a 429, a
    /// `RESOURCE_EXHAUSTED` status, or a 403 whose reason is
    /// `rateLimitExceeded`, `userRateLimitExceeded`, `quotaExceeded` or
    /// `RATE_LIMIT_EXCEEDED`.
    ///
    /// The daily quota (`dailyLimitExceeded`) is not a rate limit:
    /// waiting a minute does not lift it.
    pub fn is_rate_limited(&self) -> bool {
        if self.status == 429 || self.google_status.as_deref() == Some("RESOURCE_EXHAUSTED") {
            return true;
        }

        self.status == 403
            && [
                "rateLimitExceeded",
                "userRateLimitExceeded",
                "quotaExceeded",
                "RATE_LIMIT_EXCEEDED",
            ]
            .iter()
            .any(|reason| self.has_reason(reason))
    }

    /// Whether the request is worth sending again later: rate limited
    /// ([`Self::is_rate_limited`]) or a transient 5xx.
    pub fn is_retryable(&self) -> bool {
        self.is_rate_limited() || matches!(self.status, 500 | 502 | 503 | 504)
    }

    /// Whether the target of the request does not exist (404).
    pub fn is_not_found(&self) -> bool {
        self.status == 404
    }

    /// Whether the sync token of a listing expired: any 410 but the one
    /// whose reason is `deleted`, which speaks of a resource that is
    /// gone and not of the token.
    ///
    /// A 410 giving a reason this crate does not know, or none, reads
    /// as expired: a new baseline loses nothing.
    pub fn is_sync_token_expired(&self) -> bool {
        self.status == 410 && !self.has_reason("deleted")
    }
}

/// Terminal value of a successful Google Calendar exchange.
#[derive(Clone, Debug)]
pub struct GcalSendOutput<T> {
    /// The parsed 2xx response body.
    pub response: T,
    /// Whether the server allows reusing the TCP/TLS connection.
    pub keep_alive: bool,
}

/// I/O-free coroutine sending one authorized HTTP request and parsing
/// the JSON response into `T`.
pub struct GcalSend<T> {
    state: State,
    _phantom: PhantomData<T>,
}

impl<T: DeserializeOwned> GcalSend<T> {
    /// Builds a GET request against the given URL.
    pub fn get(auth: &HttpAuthBearer, url: Url) -> Self {
        Self::with_method(auth, "GET", url, None, Vec::new())
    }

    /// Builds a DELETE request against the given URL.
    pub fn delete(auth: &HttpAuthBearer, url: Url) -> Self {
        Self::with_method(auth, "DELETE", url, None, Vec::new())
    }

    /// Builds a DELETE request guarded by an entity tag.
    ///
    /// See [`Self::with_guard`] for what the guard does.
    pub fn delete_if_match(
        auth: &HttpAuthBearer,
        url: Url,
        if_match: Option<&str>,
    ) -> Result<Self, GcalSendError> {
        Self::with_guard(auth, "DELETE", url, None, Vec::new(), if_match)
    }

    /// Builds a POST request with no body, for the custom verbs that
    /// take all their input from the query (clear, move, quickAdd).
    pub fn post_empty(auth: &HttpAuthBearer, url: Url) -> Self {
        Self::with_method(auth, "POST", url, Some("application/json"), Vec::new())
    }

    /// Builds a POST request with the given value as JSON body.
    pub fn post_json<B: Serialize>(
        auth: &HttpAuthBearer,
        url: Url,
        body: &B,
    ) -> Result<Self, GcalSendError> {
        let body = serde_json::to_vec(body).map_err(GcalSendError::SerializeRequest)?;
        Ok(Self::with_method(
            auth,
            "POST",
            url,
            Some("application/json"),
            body,
        ))
    }

    /// Builds a PUT request with the given value as JSON body.
    pub fn put_json<B: Serialize>(
        auth: &HttpAuthBearer,
        url: Url,
        body: &B,
    ) -> Result<Self, GcalSendError> {
        let body = serde_json::to_vec(body).map_err(GcalSendError::SerializeRequest)?;
        Ok(Self::with_method(
            auth,
            "PUT",
            url,
            Some("application/json"),
            body,
        ))
    }

    /// Builds a PUT request with the given value as JSON body, guarded
    /// by an entity tag.
    ///
    /// See [`Self::with_guard`] for what the guard does.
    pub fn put_json_if_match<B: Serialize>(
        auth: &HttpAuthBearer,
        url: Url,
        body: &B,
        if_match: Option<&str>,
    ) -> Result<Self, GcalSendError> {
        let body = serde_json::to_vec(body).map_err(GcalSendError::SerializeRequest)?;
        Self::with_guard(auth, "PUT", url, Some("application/json"), body, if_match)
    }

    /// Builds a PATCH request with the given value as JSON body.
    pub fn patch_json<B: Serialize>(
        auth: &HttpAuthBearer,
        url: Url,
        body: &B,
    ) -> Result<Self, GcalSendError> {
        let body = serde_json::to_vec(body).map_err(GcalSendError::SerializeRequest)?;
        Ok(Self::with_method(
            auth,
            "PATCH",
            url,
            Some("application/json"),
            body,
        ))
    }

    /// Builds a PATCH request with the given value as JSON body,
    /// guarded by an entity tag.
    ///
    /// See [`Self::with_guard`] for what the guard does.
    pub fn patch_json_if_match<B: Serialize>(
        auth: &HttpAuthBearer,
        url: Url,
        body: &B,
        if_match: Option<&str>,
    ) -> Result<Self, GcalSendError> {
        let body = serde_json::to_vec(body).map_err(GcalSendError::SerializeRequest)?;
        Self::with_guard(auth, "PATCH", url, Some("application/json"), body, if_match)
    }

    /// Builds a request with an arbitrary method, content type and body.
    ///
    /// A method that is not an HTTP token, or a bearer token or content
    /// type holding a line break, fails the first resume with
    /// [`GcalSendError::InvalidRequest`], before any write.
    pub fn with_method(
        auth: &HttpAuthBearer,
        method: &str,
        url: Url,
        content_type: Option<&str>,
        body: Vec<u8>,
    ) -> Self {
        Self::build(auth, method, url, content_type, body, None)
    }

    /// Builds a request with an arbitrary method, content type, body
    /// and optional entity tag guard.
    ///
    /// The guard rides as an `If-Match` header carrying the etag a read
    /// returned, so the write only lands while the resource has not
    /// moved underneath it; a stale tag comes back as HTTP 412. An
    /// absent guard overwrites unconditionally.
    ///
    /// A guard holding a control character is refused: it would end
    /// the header line it rides on.
    pub fn with_guard(
        auth: &HttpAuthBearer,
        method: &str,
        url: Url,
        content_type: Option<&str>,
        body: Vec<u8>,
        if_match: Option<&str>,
    ) -> Result<Self, GcalSendError> {
        if if_match.is_some_and(|tag| tag.contains(char::is_control)) {
            let reason = "Entity tag holds a control character";
            return Err(GcalSendError::InvalidRequest(reason.into()));
        }

        Ok(Self::build(auth, method, url, content_type, body, if_match))
    }

    fn build(
        auth: &HttpAuthBearer,
        method: &str,
        url: Url,
        content_type: Option<&str>,
        body: Vec<u8>,
        if_match: Option<&str>,
    ) -> Self {
        let host = url.host_str().unwrap_or("localhost");

        let mut request = HttpRequest::get(url.clone())
            .header("Host", host)
            .header("Accept", "application/json")
            .header("Authorization", auth.to_authorization())
            .body(body);

        if let Some(content_type) = content_type {
            request = request.header("Content-Type", content_type);
        }

        if let Some(if_match) = if_match {
            request = request.header("If-Match", if_match);
        }

        request.method = method.into();

        debug!("prepare request to send");
        trace!("method: {method}");
        trace!("url: {url}");
        trace!("if_match: {if_match:?}");

        Self {
            state: State::Send(Http11Send::new(request)),
            _phantom: PhantomData,
        }
    }
}

impl<T: DeserializeOwned> GcalCoroutine for GcalSend<T> {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<T>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        match &mut self.state {
            State::Send(send) => match send.resume(arg) {
                HttpCoroutineState::Yielded(HttpSendYield::WantsRead) => {
                    GcalCoroutineState::Yielded(GcalYield::WantsRead)
                }
                HttpCoroutineState::Yielded(HttpSendYield::WantsWrite(bytes)) => {
                    GcalCoroutineState::Yielded(GcalYield::WantsWrite(bytes))
                }
                HttpCoroutineState::Yielded(HttpSendYield::WantsRedirect { .. }) => {
                    GcalCoroutineState::Complete(Err(GcalSendError::UnexpectedRedirect))
                }
                HttpCoroutineState::Complete(Err(Http11SendError::InvalidRequest(err))) => {
                    let err = GcalSendError::InvalidRequest(err.to_string());
                    GcalCoroutineState::Complete(Err(err))
                }
                HttpCoroutineState::Complete(Err(err)) => {
                    GcalCoroutineState::Complete(Err(err.into()))
                }
                HttpCoroutineState::Complete(Ok(HttpSendOutput {
                    response,
                    keep_alive,
                    ..
                })) => {
                    if response.status.is_success() {
                        // NOTE: an empty 2xx body — a DELETE, a clear or
                        // a channel stop — is normalised to `{}`.
                        // `GcalNoResponse` ignores it and struct
                        // responses fall back to their
                        // `#[serde(default)]` fields; `null` would fail
                        // every struct response with "invalid type:
                        // null".
                        let body = if response.body.is_empty() {
                            b"{}".as_slice()
                        } else {
                            response.body.as_slice()
                        };

                        match serde_json::from_slice::<T>(body) {
                            Ok(response) => GcalCoroutineState::Complete(Ok(GcalSendOutput {
                                response,
                                keep_alive,
                            })),
                            Err(err) => {
                                GcalCoroutineState::Complete(Err(GcalSendError::ParseResponse(err)))
                            }
                        }
                    } else {
                        let mut err = GcalApiError::parse(*response.status, &response.body);
                        let delay = response.header("retry-after").and_then(parse_delay);

                        err.retry_after = delay.or(err.retry_after);
                        err.keep_alive = keep_alive;

                        GcalCoroutineState::Complete(Err(GcalSendError::Api(err)))
                    }
                }
            },
        }
    }
}

enum State {
    Send(Http11Send),
}

#[derive(Debug, Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    code: Option<u16>,
    message: Option<String>,
    #[serde(default)]
    errors: Vec<ErrorItem>,
    status: Option<String>,
    #[serde(default)]
    details: Vec<ErrorItem>,
}

/// An item of `error.errors[]` or `error.details[]`, of which only the
/// reason and the retry delay are kept.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorItem {
    reason: Option<String>,
    // NOTE: read loosely, so an unexpected shape cannot fail the envelope.
    retry_delay: Option<Value>,
}

/// Reads a delay in seconds: the `Retry-After` header (`120`) or the
/// `retryDelay` of the envelope (`1.5s`). A date reads as none.
fn parse_delay(value: &str) -> Option<Duration> {
    let value = value.trim();
    let value = value.strip_suffix('s').unwrap_or(value);
    let (secs, frac) = value.split_once('.').unwrap_or((value, ""));

    if secs.is_empty() || !secs.chars().chain(frac.chars()).all(|c| c.is_ascii_digit()) {
        return None;
    }

    let secs = secs.parse().ok()?;
    let nanos = frac.bytes().chain([b'0'; 9]).take(9);
    let nanos = nanos.fold(0, |nanos, digit| nanos * 10 + u32::from(digit - b'0'));

    Some(Duration::new(secs, nanos))
}

/// Builds the Calendar API URL of a path from its segments.
///
/// Each segment is percent-encoded on its own, so an id holding `/`,
/// `?` or `#` names no other resource. A segment the URL would drop or
/// rewrite is refused: an empty one, `.`, `..`, or one holding a tab
/// or a line break.
pub fn gcal_api_url(segments: &[&str]) -> Result<Url, GcalSendError> {
    http_url_build(GCAL_API_BASE, segments).map_err(|err| match err {
        HttpUrlBuildError::InvalidBaseUrl(err, _) => GcalSendError::ParseUrl(err),
        HttpUrlBuildError::InvalidSegment(segment) => {
            let reason = format!("Path segment {segment:?} names no resource");
            GcalSendError::InvalidRequest(reason)
        }
    })
}

/// Parses Calendar's JSON error envelope, falling back to the raw body;
/// returns the effective status code and message.
pub fn parse_api_error(http_status: u16, body: &[u8]) -> (u16, String) {
    if let Ok(envelope) = serde_json::from_slice::<ErrorEnvelope>(body) {
        let status = envelope.error.code.unwrap_or(http_status);
        let message = envelope
            .error
            .message
            .filter(|message| !message.trim().is_empty())
            .unwrap_or_else(|| String::from("unknown Calendar API error"));
        return (status, message);
    }

    let message = String::from_utf8_lossy(body).trim().to_string();

    if message.is_empty() {
        (http_status, String::from("unknown Calendar API error"))
    } else {
        (http_status, message)
    }
}
