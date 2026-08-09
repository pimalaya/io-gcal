//! Offline coverage of the transport every coroutine delegates to:
//! the authorized request builder, the JSON parsing, the error
//! envelope, the redirect refusal and the keep-alive flag.

mod common;

use common::*;
use io_gcal::v3::{
    rest::{
        calendars::{delete::GcalCalendarDelete, get::GcalCalendarGet},
        colors::get::GcalColorsGet,
    },
    send::{GCAL_API_BASE, GcalSendError, parse_api_error},
};

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

        let err = GcalSendError::Api {
            status,
            message: message.clone(),
        };
        assert!(err.is_retryable(), "{status} should be retryable");
        assert!(!err.is_sync_token_expired());
    }

    for status in [400, 401, 403, 404, 409, 410] {
        let err = GcalSendError::Api {
            status,
            message: String::from("nope"),
        };
        assert!(!err.is_retryable(), "{status} should not be retryable");
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
