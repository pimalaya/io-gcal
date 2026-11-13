//! Offline coverage of the transport every coroutine delegates to:
//! the authorized request builder, the JSON parsing, the error
//! envelope, the redirect refusal, the refusal of a request that would
//! split and the keep-alive flag.

mod common;

use core::{fmt::Debug, time::Duration};

use common::*;
use io_gcal::{
    coroutine::*,
    v3::{
        rest::{
            calendars::{delete::GcalCalendarDelete, get::GcalCalendarGet},
            colors::get::GcalColorsGet,
        },
        send::{
            GCAL_API_BASE, GcalApiError, GcalNoResponse, GcalSend, GcalSendError, gcal_api_url,
            parse_api_error,
        },
    },
};
use io_http::rfc6750::bearer::HttpAuthBearer;

#[test]
fn targets_the_versioned_api_base() {
    assert_eq!(GCAL_API_BASE, "https://www.googleapis.com/calendar/v3/");
}

#[test]
fn authorizes_every_request() {
    let mut coroutine = GcalColorsGet::new(&auth()).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", "{}"));

    ret.unwrap();

    assert!(
        request.contains("Authorization: Bearer fake-token"),
        "got: {request}"
    );
    assert!(
        request.contains("Accept: application/json"),
        "got: {request}"
    );
    assert!(
        request.contains("Host: www.googleapis.com"),
        "got: {request}"
    );
}

#[test]
fn parses_an_empty_body_into_no_response() {
    let mut coroutine = GcalCalendarDelete::new(&auth(), "primary").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &empty_response("204 No Content"));

    ret.unwrap();
}

#[test]
fn reports_a_reusable_connection() {
    let mut coroutine = GcalColorsGet::new(&auth()).unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", "{}"));

    assert!(ret.unwrap().keep_alive);
}

#[test]
fn reports_a_closed_connection() {
    let response = http_response("200 OK", &[("Connection", "close")], "{}");
    let mut coroutine = GcalColorsGet::new(&auth()).unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &response);

    assert!(!ret.unwrap().keep_alive);
}

#[test]
fn surfaces_the_error_envelope() {
    let response = json_response(
        "403 Forbidden",
        r#"{"error":{"code":403,"message":"Insufficient Permission","errors":[]}}"#,
    );
    let mut coroutine = GcalCalendarGet::new(&auth(), "primary").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &response);
    let Err(err) = ret else {
        panic!("expected an API error");
    };

    assert_eq!(err.status(), Some(403));
    assert!(err.to_string().contains("Insufficient Permission"));
    assert!(!err.is_retryable());
}

#[test]
fn prefers_the_envelope_status_over_the_http_one() {
    let response = json_response(
        "400 Bad Request",
        r#"{"error":{"code":410,"message":"Gone"}}"#,
    );
    let mut coroutine = GcalCalendarGet::new(&auth(), "primary").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &response);
    let Err(err) = ret else {
        panic!("expected an API error");
    };

    assert_eq!(err.status(), Some(410));
    assert!(err.is_sync_token_expired());
}

#[test]
fn falls_back_to_the_raw_error_body() {
    let response = http_response("502 Bad Gateway", &[], "upstream exploded");
    let mut coroutine = GcalCalendarGet::new(&auth(), "primary").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &response);
    let Err(err) = ret else {
        panic!("expected an API error");
    };

    assert_eq!(err.status(), Some(502));
    assert!(err.to_string().contains("upstream exploded"));
    assert!(err.is_retryable());
}

#[test]
fn refuses_to_follow_a_redirect() {
    let response = http_response(
        "302 Found",
        &[("Location", "https://elsewhere.example.org/")],
        "",
    );
    let mut coroutine = GcalCalendarGet::new(&auth(), "primary").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &response);
    let Err(err) = ret else {
        panic!("expected a redirect refusal");
    };

    assert!(
        matches!(err, GcalSendError::UnexpectedRedirect),
        "got: {err}"
    );
    assert_eq!(err.status(), None);
}

/// Resumes a coroutine once and returns the reason it gives for
/// refusing its own request, before any write.
fn refusal<C, R>(mut coroutine: C) -> String
where
    C: GcalCoroutine<Yield = GcalYield, Return = Result<R, GcalSendError>>,
    R: Debug,
{
    match coroutine.resume(None) {
        GcalCoroutineState::Complete(Err(GcalSendError::InvalidRequest(reason))) => reason,
        state => panic!("expected InvalidRequest, got {state:?}"),
    }
}

#[test]
fn refuses_a_token_a_method_or_a_content_type_holding_a_line_break() {
    let split = HttpAuthBearer::new("fake-token\r\nX-Injected: 1");
    let reason = refusal(GcalColorsGet::new(&split).unwrap());

    assert!(reason.contains("Authorization"), "got: {reason}");
    assert!(!reason.contains("fake-token"), "got: {reason}");

    let url = gcal_api_url(&["colors"]).unwrap();
    let method = "GET / HTTP/1.1\r\nX-Injected: 1\r\n\r\nGET";
    let send = GcalSend::<GcalNoResponse>::with_method(&auth(), method, url.clone(), None, vec![]);

    assert!(refusal(send).contains("method"));

    let content_type = Some("application/json\r\nX-Injected: 1");
    let send = GcalSend::<GcalNoResponse>::with_method(&auth(), "POST", url, content_type, vec![]);

    assert!(refusal(send).contains("Content-Type"));
}

#[test]
fn surfaces_a_broken_http_reply() {
    let mut coroutine = GcalCalendarGet::new(&auth(), "primary").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, b"definitely not HTTP\r\n\r\n");
    let Err(err) = ret else {
        panic!("expected a transport error");
    };

    assert!(matches!(err, GcalSendError::Send(_)), "got: {err}");
    assert_eq!(err.status(), None);
    assert!(!err.is_retryable());
}

#[test]
fn reports_an_unparsable_success_body() {
    let response = json_response("200 OK", "{not json");
    let mut coroutine = GcalCalendarGet::new(&auth(), "primary").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &response);
    let Err(err) = ret else {
        panic!("expected a parse error");
    };

    assert!(matches!(err, GcalSendError::ParseResponse(_)), "got: {err}");
}

#[test]
fn flags_the_transient_statuses_as_retryable() {
    for status in [429, 500, 502, 503, 504] {
        let (_, message) = parse_api_error(status, b"");
        assert_eq!(message, "unknown Calendar API error");

        let err = GcalSendError::Api(GcalApiError {
            status,
            message: message.clone(),
            ..Default::default()
        });
        assert!(err.is_retryable(), "{status} should be retryable");
        assert!(!err.is_sync_token_expired());
    }

    for status in [400, 401, 403, 404, 409, 410] {
        let err = GcalSendError::Api(GcalApiError {
            status,
            message: String::from("nope"),
            ..Default::default()
        });
        assert!(!err.is_retryable(), "{status} should not be retryable");
    }
}

#[test]
fn flags_a_rate_limit_reason_as_retryable() {
    let err = api_error(
        "403 Forbidden",
        &[],
        r#"{"error":{"code":403,"message":"Rate Limit Exceeded","errors":[{"domain":"usageLimits","reason":"rateLimitExceeded","message":"Rate Limit Exceeded"}],"status":"PERMISSION_DENIED","details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"RATE_LIMIT_EXCEEDED"}]}}"#,
    );
    let api = err.api().unwrap();

    assert_eq!(api.reasons, ["rateLimitExceeded"]);
    assert_eq!(api.google_status.as_deref(), Some("PERMISSION_DENIED"));
    assert_eq!(api.detail_reasons, ["RATE_LIMIT_EXCEEDED"]);
    assert!(api.has_reason("rateLimitExceeded"));
    assert!(err.is_rate_limited());
    assert!(err.is_retryable());
    assert!(err.to_string().contains("HTTP 403: Rate Limit Exceeded"));
}

#[test]
fn tells_a_rate_limit_from_another_refusal() {
    let error = |code: u16, reason: &str, status: &str| {
        GcalApiError::parse(
            code,
            format!(
                r#"{{"error":{{"code":{code},"message":"no","errors":[{{"reason":"{reason}"}}],"status":"{status}"}}}}"#
            )
            .as_bytes(),
        )
    };

    assert!(error(403, "userRateLimitExceeded", "PERMISSION_DENIED").is_rate_limited());
    assert!(error(403, "quotaExceeded", "PERMISSION_DENIED").is_rate_limited());
    assert!(error(400, "badRequest", "RESOURCE_EXHAUSTED").is_rate_limited());
    assert!(error(429, "badRequest", "UNKNOWN").is_rate_limited());

    // NOTE: waiting does not lift the daily quota.
    let daily = error(403, "dailyLimitExceeded", "PERMISSION_DENIED");
    assert!(daily.has_reason("dailyLimitExceeded"));
    assert!(!daily.is_rate_limited());
    assert!(!daily.is_retryable());

    let denied = error(403, "insufficientPermissions", "PERMISSION_DENIED");
    assert!(!denied.is_rate_limited());

    let missing = error(404, "notFound", "NOT_FOUND");
    assert!(missing.is_not_found());
    assert!(!missing.is_retryable());
}

#[test]
fn tells_a_deleted_resource_from_an_expired_sync_token() {
    let gone = |reason: &str| {
        let body = format!(
            r#"{{"error":{{"code":410,"message":"Gone","errors":[{{"domain":"global","reason":"{reason}","message":"Gone"}}]}}}}"#
        );
        api_error("410 Gone", &[], &body)
    };

    assert!(!gone("deleted").is_sync_token_expired());

    // NOTE: every other 410 reads as an expired token, a reason this
    // crate does not know and no reason at all included: a new baseline
    // loses nothing, a missed expiry stalls the sync for good.
    for reason in ["fullSyncRequired", "updatedMinTooLongAgo", "somethingNew"] {
        assert!(gone(reason).is_sync_token_expired(), "{reason}");
    }

    assert!(api_error("410 Gone", &[], "").is_sync_token_expired());
    assert!(!api_error("404 Not Found", &[], "").is_sync_token_expired());
}

#[test]
fn carries_the_delay_of_the_retry_after_header() {
    let err = api_error("429 Too Many Requests", &[("Retry-After", "120")], "busy");

    assert_eq!(err.retry_after(), Some(Duration::from_secs(120)));
}

#[test]
fn carries_the_delay_of_the_envelope() {
    let body = r#"{"error":{"code":429,"message":"busy","status":"RESOURCE_EXHAUSTED","details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"RATE_LIMIT_EXCEEDED"},{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"1.5s"}]}}"#;
    let err = api_error("429 Too Many Requests", &[], body);

    assert_eq!(err.retry_after(), Some(Duration::from_millis(1500)));
    assert_eq!(err.api().unwrap().detail_reasons, ["RATE_LIMIT_EXCEEDED"]);

    // NOTE: the header wins over the envelope.
    let err = api_error("429 Too Many Requests", &[("Retry-After", "7")], body);
    assert_eq!(err.retry_after(), Some(Duration::from_secs(7)));
}

#[test]
fn reads_no_delay_out_of_a_date_or_a_malformed_value() {
    for value in ["Fri, 13 Nov 2026 12:00:00 GMT", "-5", "+5", "soon", ""] {
        let err = api_error("503 Service Unavailable", &[("Retry-After", value)], "");
        assert_eq!(err.retry_after(), None, "got a delay out of {value:?}");
    }

    let err = api_error("503 Service Unavailable", &[], "");
    assert_eq!(err.retry_after(), None);

    // NOTE: a delay of another shape must not cost the envelope.
    let body =
        r#"{"error":{"code":429,"message":"busy","details":[{"retryDelay":{"seconds":30}}]}}"#;
    let err = api_error("400 Bad Request", &[], body);
    assert_eq!(err.status(), Some(429));
    assert_eq!(err.retry_after(), None);
}

#[test]
fn reports_whether_a_refusal_leaves_the_connection_reusable() {
    let err = api_error("404 Not Found", &[("Connection", "keep-alive")], "");
    assert!(err.api().unwrap().keep_alive);
    assert!(err.is_not_found());

    let err = api_error("404 Not Found", &[("Connection", "close")], "");
    assert!(!err.api().unwrap().keep_alive);
}

/// Runs an exchange answered by a non-2xx response and returns its
/// error.
fn api_error(status: &str, headers: &[(&str, &str)], body: &str) -> GcalSendError {
    let response = http_response(status, headers, body);
    let mut coroutine = GcalCalendarGet::new(&auth(), "primary").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &response);

    match ret {
        Err(err) => err,
        Ok(_) => panic!("expected an API error"),
    }
}

#[test]
fn parses_error_bodies() {
    let (status, message) =
        parse_api_error(400, br#"{"error":{"code":403,"message":"Forbidden"}}"#);
    assert_eq!(status, 403);
    assert_eq!(message, "Forbidden");

    // NOTE: an envelope without a code keeps the HTTP status, and a
    // blank message falls back rather than surfacing as an empty error.
    let (status, message) = parse_api_error(418, br#"{"error":{"message":"   "}}"#);
    assert_eq!(status, 418);
    assert_eq!(message, "unknown Calendar API error");

    let (status, message) = parse_api_error(500, b"  boom  ");
    assert_eq!(status, 500);
    assert_eq!(message, "boom");

    let (status, message) = parse_api_error(503, b"");
    assert_eq!(status, 503);
    assert_eq!(message, "unknown Calendar API error");
}
