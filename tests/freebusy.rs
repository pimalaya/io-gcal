//! Offline coverage of the free/busy query (`freebusy`): the request
//! body, the per-calendar answer and the per-calendar errors.

mod common;

use common::*;
use io_gcal::v3::{
    rest::freebusy::{
        GcalFreeBusyRequest, GcalFreeBusyRequestItem, GcalFreeBusyResponse,
        query::GcalFreeBusyQuery,
    },
    send::GcalSendError,
};

fn request() -> GcalFreeBusyRequest {
    GcalFreeBusyRequest {
        time_min: Some(String::from("2026-08-01T00:00:00Z")),
        time_max: Some(String::from("2026-08-02T00:00:00Z")),
        items: vec![GcalFreeBusyRequestItem {
            id: Some(String::from("primary")),
        }],
        ..Default::default()
    }
}

#[test]
fn queries_the_busy_periods() {
    let body = r#"{"kind":"calendar#freeBusy","timeMin":"2026-08-01T00:00:00Z","timeMax":"2026-08-02T00:00:00Z","calendars":{"primary":{"busy":[{"start":"2026-08-01T09:00:00Z","end":"2026-08-01T10:00:00Z"}]}}}"#;
    let mut coroutine = GcalFreeBusyQuery::new(&auth(), &request()).unwrap();
    let (http, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", body));
    let out = ret.unwrap();

    let busy = &out.response.calendars["primary"].busy;
    assert_eq!(busy.len(), 1);
    assert_eq!(busy[0].start.as_deref(), Some("2026-08-01T09:00:00Z"));
    assert_eq!(busy[0].end.as_deref(), Some("2026-08-01T10:00:00Z"));

    assert_eq!(request_line(&http), "POST /calendar/v3/freeBusy");
    assert_eq!(
        request_body(&http),
        r#"{"timeMin":"2026-08-01T00:00:00Z","timeMax":"2026-08-02T00:00:00Z","items":[{"id":"primary"}]}"#
    );
}

#[test]
fn sends_the_expansion_caps() {
    let query = GcalFreeBusyRequest {
        group_expansion_max: Some(100),
        calendar_expansion_max: Some(50),
        time_zone: Some(String::from("Europe/Paris")),
        ..request()
    };
    let mut coroutine = GcalFreeBusyQuery::new(&auth(), &query).unwrap();
    let (http, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", "{}"));

    ret.unwrap();

    let body = request_body(&http);
    assert!(body.contains(r#""groupExpansionMax":100"#), "got: {body}");
    assert!(body.contains(r#""calendarExpansionMax":50"#), "got: {body}");
    assert!(body.contains(r#""timeZone":"Europe/Paris""#), "got: {body}");
}

#[test]
fn refuses_a_query_without_any_calendar() {
    let Err(err) = GcalFreeBusyQuery::new(&auth(), &GcalFreeBusyRequest::default()) else {
        panic!("expected an invalid request error");
    };

    assert!(
        matches!(err, GcalSendError::InvalidRequest(ref message) if message.contains("calendar")),
        "got: {err}"
    );
}

#[test]
fn parses_the_per_calendar_and_per_group_errors() {
    let body = r#"{"calendars":{"missing@example.org":{"busy":[],"errors":[{"domain":"calendar","reason":"notFound"}]}},"groups":{"team@example.org":{"calendars":["a@example.org"],"errors":[{"domain":"calendar","reason":"groupTooBig"}]}}}"#;
    let response: GcalFreeBusyResponse = serde_json::from_str(body).unwrap();

    let calendar = &response.calendars["missing@example.org"];
    assert!(calendar.busy.is_empty());
    assert_eq!(calendar.errors[0].reason.as_deref(), Some("notFound"));

    let group = &response.groups["team@example.org"];
    assert_eq!(group.calendars, vec![String::from("a@example.org")]);
    assert_eq!(group.errors[0].reason.as_deref(), Some("groupTooBig"));
}
