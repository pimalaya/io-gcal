//! Offline coverage of the standard blocking client: its methods run
//! against a scripted stream replaying canned HTTP responses, plus the
//! error paths a real stream can take.

#![cfg(feature = "client")]

mod common;

use std::{
    collections::VecDeque,
    io::{Error as IoError, Read, Result as IoResult, Write},
};

use common::*;
use io_gcal::v3::{
    client::{GcalClientStd, GcalClientStdError},
    rest::{
        acl::{GcalAccessRole, GcalAclRule, GcalAclScope, GcalAclScopeType},
        calendar_list::GcalCalendarListEntry,
        calendars::GcalCalendar,
        channels::{GcalChannel, GcalChannelType},
        events::{GcalEvent, GcalEventDateTime, GcalSendUpdates},
        freebusy::{GcalFreeBusyRequest, GcalFreeBusyRequestItem},
    },
};

/// Stream replaying canned HTTP responses: each read pops and serves
/// the next response whole; writes are recorded so a test can assert on
/// the requests the client produced.
struct ScriptedStream {
    responses: VecDeque<Vec<u8>>,
    written: Vec<u8>,
}

impl ScriptedStream {
    fn new(responses: impl IntoIterator<Item = Vec<u8>>) -> Self {
        Self {
            responses: responses.into_iter().collect(),
            written: Vec::new(),
        }
    }
}

impl Read for ScriptedStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let Some(response) = self.responses.pop_front() else {
            return Ok(0);
        };

        assert!(response.len() <= buf.len(), "scripted response too large");
        buf[..response.len()].copy_from_slice(&response);
        Ok(response.len())
    }
}

impl Write for ScriptedStream {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.written.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

/// Stream failing every read, to surface I/O errors out of the client.
struct FailingStream;

impl Read for FailingStream {
    fn read(&mut self, _: &mut [u8]) -> IoResult<usize> {
        Err(IoError::other("scripted read failure"))
    }
}

impl Write for FailingStream {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

/// Client whose stream serves the given responses in order.
fn client(responses: Vec<Vec<u8>>) -> GcalClientStd {
    GcalClientStd::new(ScriptedStream::new(responses), "fake-token")
}

/// A channel good enough for the watch methods to accept.
fn channel() -> GcalChannel {
    GcalChannel {
        id: Some(String::from("chan-1")),
        channel_type: Some(GcalChannelType::WebHook),
        address: Some(String::from("https://hook.example.org/gcal")),
        ..Default::default()
    }
}

/// The requests the client wrote to its scripted stream.
fn written(client: &mut GcalClientStd) -> String {
    let stream = client
        .stream
        .as_any_mut()
        .downcast_mut::<ScriptedStream>()
        .expect("the client still owns its scripted stream");

    String::from_utf8_lossy(&stream.written).into_owned()
}

#[test]
fn walks_the_calendar_list() {
    let mut client = client(vec![
        json_response("200 OK", r#"{"items":[{"id":"primary","primary":true}]}"#),
        json_response("200 OK", r#"{"id":"primary","summary":"Jane"}"#),
    ]);

    let listed = client
        .calendar_list_list(&Default::default())
        .expect("calendar list list")
        .response;
    assert_eq!(listed.items.len(), 1);

    let entry = client
        .calendar_list_entry_get("primary")
        .expect("calendar list entry get")
        .response;
    assert_eq!(entry.summary.as_deref(), Some("Jane"));

    let requests = written(&mut client);
    assert!(
        requests.contains("GET /calendar/v3/users/me/calendarList HTTP/1.1"),
        "got: {requests}"
    );
    assert!(
        requests.contains("GET /calendar/v3/users/me/calendarList/primary HTTP/1.1"),
        "got: {requests}"
    );
}

#[test]
fn walks_a_calendar_lifecycle() {
    let calendar = r#"{"id":"cal-1","summary":"Team"}"#;
    let mut client = client(vec![
        json_response("200 OK", calendar),
        json_response("200 OK", calendar),
        json_response("200 OK", calendar),
        json_response("200 OK", calendar),
        empty_response("204 No Content"),
        empty_response("204 No Content"),
    ]);

    let created = GcalCalendar {
        summary: Some(String::from("Team")),
        ..Default::default()
    };

    client.calendar_insert(&created).expect("calendar insert");
    client.calendar_get("cal-1").expect("calendar get");
    client
        .calendar_update("cal-1", &created)
        .expect("calendar update");
    client
        .calendar_patch("cal-1", &created)
        .expect("calendar patch");
    client.calendar_clear("cal-1").expect("calendar clear");
    client.calendar_delete("cal-1").expect("calendar delete");

    let requests = written(&mut client);
    assert!(requests.contains("POST /calendar/v3/calendars HTTP/1.1"));
    assert!(requests.contains("GET /calendar/v3/calendars/cal-1 HTTP/1.1"));
    assert!(requests.contains("PUT /calendar/v3/calendars/cal-1 HTTP/1.1"));
    assert!(requests.contains("PATCH /calendar/v3/calendars/cal-1 HTTP/1.1"));
    assert!(requests.contains("POST /calendar/v3/calendars/cal-1/clear HTTP/1.1"));
    assert!(requests.contains("DELETE /calendar/v3/calendars/cal-1 HTTP/1.1"));
}

#[test]
fn walks_an_event_lifecycle() {
    let event = r#"{"id":"ev1","summary":"Review"}"#;
    let mut client = client(vec![
        json_response("200 OK", event),
        json_response("200 OK", event),
        json_response("200 OK", event),
        json_response("200 OK", r#"{"items":[{"id":"ev1"}]}"#),
        json_response("200 OK", r#"{"items":[{"id":"ev1_20260811"}]}"#),
        json_response("200 OK", event),
        json_response("200 OK", event),
        empty_response("204 No Content"),
    ]);

    let event_body = GcalEvent {
        summary: Some(String::from("Review")),
        start: Some(GcalEventDateTime {
            date_time: Some(String::from("2026-08-11T10:00:00Z")),
            ..Default::default()
        }),
        end: Some(GcalEventDateTime {
            date_time: Some(String::from("2026-08-11T11:00:00Z")),
            ..Default::default()
        }),
        ..Default::default()
    };

    client
        .event_insert("primary", &event_body, &Default::default())
        .expect("event insert");
    client
        .event_get("primary", "ev1", None, None)
        .expect("event get");
    client
        .event_patch("primary", "ev1", &event_body, &Default::default())
        .expect("event patch");
    client
        .events_list("primary", &Default::default())
        .expect("events list");
    client
        .event_instances("primary", "ev1", &Default::default())
        .expect("event instances");
    client
        .event_move("primary", "ev1", "other@example.org", None)
        .expect("event move");
    client
        .event_quick_add(
            "primary",
            "Lunch tomorrow 12pm",
            Some(GcalSendUpdates::None),
        )
        .expect("event quick add");
    client
        .event_delete("primary", "ev1", None)
        .expect("event delete");

    let requests = written(&mut client);
    assert!(requests.contains("POST /calendar/v3/calendars/primary/events HTTP/1.1"));
    assert!(requests.contains("GET /calendar/v3/calendars/primary/events/ev1 HTTP/1.1"));
    assert!(requests.contains("PATCH /calendar/v3/calendars/primary/events/ev1 HTTP/1.1"));
    assert!(requests.contains("GET /calendar/v3/calendars/primary/events HTTP/1.1"));
    assert!(requests.contains("/events/ev1/instances HTTP/1.1"));
    assert!(requests.contains("/events/ev1/move?destination=other%40example.org HTTP/1.1"));
    assert!(requests.contains("/events/quickAdd?text=Lunch+tomorrow+12pm&sendUpdates=none"));
    assert!(requests.contains("DELETE /calendar/v3/calendars/primary/events/ev1 HTTP/1.1"));
}

#[test]
fn reads_the_account_wide_resources() {
    let mut client = client(vec![
        json_response("200 OK", r##"{"event":{"1":{"background":"#a4bdfc"}}}"##),
        json_response("200 OK", r#"{"items":[{"id":"timezone","value":"UTC"}]}"#),
        json_response("200 OK", r#"{"id":"timezone","value":"UTC"}"#),
        json_response("200 OK", r#"{"calendars":{"primary":{"busy":[]}}}"#),
    ]);

    assert_eq!(
        client
            .colors_get()
            .expect("colors get")
            .response
            .event
            .len(),
        1
    );
    assert_eq!(
        client
            .settings_list(&Default::default())
            .expect("settings list")
            .response
            .items
            .len(),
        1
    );
    assert_eq!(
        client
            .setting_get("timezone")
            .expect("setting get")
            .response
            .value
            .as_deref(),
        Some("UTC")
    );

    let request = GcalFreeBusyRequest {
        time_min: Some(String::from("2026-08-01T00:00:00Z")),
        time_max: Some(String::from("2026-08-02T00:00:00Z")),
        items: vec![GcalFreeBusyRequestItem {
            id: Some(String::from("primary")),
        }],
        ..Default::default()
    };
    assert!(
        client
            .free_busy_query(&request)
            .expect("free/busy query")
            .response
            .calendars
            .contains_key("primary")
    );
}

#[test]
fn opens_and_closes_a_notification_channel() {
    let mut client = client(vec![
        json_response("200 OK", r#"{"id":"chan-1","resourceId":"res-1"}"#),
        empty_response("204 No Content"),
    ]);

    let channel = GcalChannel {
        id: Some(String::from("chan-1")),
        channel_type: Some(GcalChannelType::WebHook),
        address: Some(String::from("https://hook.example.org/gcal")),
        ..Default::default()
    };

    let opened = client
        .events_watch("primary", &channel, &Default::default())
        .expect("events watch")
        .response;
    assert_eq!(opened.resource_id.as_deref(), Some("res-1"));

    client.channel_stop(&opened).expect("channel stop");

    let requests = written(&mut client);
    assert!(requests.contains("POST /calendar/v3/calendars/primary/events/watch HTTP/1.1"));
    assert!(requests.contains("POST /calendar/v3/channels/stop HTTP/1.1"));
}

#[test]
fn manages_the_sharing_of_a_calendar() {
    let mut client = client(vec![
        json_response("200 OK", r#"{"items":[{"id":"user:jane@example.org"}]}"#),
        json_response(
            "200 OK",
            r#"{"id":"user:jane@example.org","role":"reader"}"#,
        ),
        empty_response("204 No Content"),
    ]);

    let listed = client
        .acl_list("primary", &Default::default())
        .expect("acl list")
        .response;
    assert_eq!(listed.items.len(), 1);

    client
        .acl_rule_get("primary", "user:jane@example.org")
        .expect("acl rule get");
    client
        .acl_rule_delete("primary", "user:jane@example.org")
        .expect("acl rule delete");

    let requests = written(&mut client);
    assert!(requests.contains("GET /calendar/v3/calendars/primary/acl HTTP/1.1"));
    assert!(requests.contains("DELETE /calendar/v3/calendars/primary/acl/user:jane@example.org"));
}

#[test]
fn writes_the_calendar_list() {
    let entry_body = r#"{"id":"cal-1","summaryOverride":"Mine"}"#;
    let mut client = client(vec![
        json_response("200 OK", entry_body),
        json_response("200 OK", entry_body),
        json_response("200 OK", entry_body),
        empty_response("204 No Content"),
        json_response("200 OK", r#"{"id":"chan-1","resourceId":"res-1"}"#),
    ]);

    let entry = GcalCalendarListEntry {
        id: Some(String::from("cal-1")),
        summary_override: Some(String::from("Mine")),
        ..Default::default()
    };

    client
        .calendar_list_entry_insert(&entry, Some(true))
        .expect("calendar list entry insert");
    client
        .calendar_list_entry_update("cal-1", &entry, None)
        .expect("calendar list entry update");
    client
        .calendar_list_entry_patch("cal-1", &entry, Some(false))
        .expect("calendar list entry patch");
    client
        .calendar_list_entry_delete("cal-1")
        .expect("calendar list entry delete");
    client
        .calendar_list_watch(&channel(), &Default::default())
        .expect("calendar list watch");

    let requests = written(&mut client);
    assert!(requests.contains("POST /calendar/v3/users/me/calendarList?colorRgbFormat=true"));
    assert!(requests.contains("PUT /calendar/v3/users/me/calendarList/cal-1 HTTP/1.1"));
    assert!(
        requests.contains("PATCH /calendar/v3/users/me/calendarList/cal-1?colorRgbFormat=false")
    );
    assert!(requests.contains("DELETE /calendar/v3/users/me/calendarList/cal-1 HTTP/1.1"));
    assert!(requests.contains("POST /calendar/v3/users/me/calendarList/watch HTTP/1.1"));
}

#[test]
fn writes_the_sharing_of_a_calendar() {
    let rule_body = r#"{"id":"user:jane@example.org","role":"reader"}"#;
    let mut client = client(vec![
        json_response("200 OK", rule_body),
        json_response("200 OK", rule_body),
        json_response("200 OK", rule_body),
        json_response("200 OK", r#"{"id":"chan-1","resourceId":"res-1"}"#),
    ]);

    let rule = GcalAclRule {
        role: Some(GcalAccessRole::Reader),
        scope: Some(GcalAclScope {
            scope_type: Some(GcalAclScopeType::Default),
            value: None,
        }),
        ..Default::default()
    };

    client
        .acl_rule_insert("primary", &rule, Some(false))
        .expect("acl rule insert");
    client
        .acl_rule_update("primary", "default", &rule, None)
        .expect("acl rule update");
    client
        .acl_rule_patch("primary", "default", &rule, Some(true))
        .expect("acl rule patch");
    client
        .acl_watch("primary", &channel(), &Default::default())
        .expect("acl watch");

    let requests = written(&mut client);
    assert!(requests.contains("POST /calendar/v3/calendars/primary/acl?sendNotifications=false"));
    assert!(requests.contains("PUT /calendar/v3/calendars/primary/acl/default HTTP/1.1"));
    assert!(
        requests
            .contains("PATCH /calendar/v3/calendars/primary/acl/default?sendNotifications=true")
    );
    assert!(requests.contains("POST /calendar/v3/calendars/primary/acl/watch HTTP/1.1"));
}

#[test]
fn covers_the_remaining_verbs() {
    let event_body = r#"{"id":"ev1"}"#;
    let mut client = client(vec![
        json_response("200 OK", event_body),
        json_response("200 OK", event_body),
        json_response("200 OK", r#"{"id":"chan-1","resourceId":"res-1"}"#),
        empty_response("204 No Content"),
    ]);

    let event = GcalEvent {
        ical_uid: Some(String::from("imported@example.org")),
        start: Some(GcalEventDateTime {
            date_time: Some(String::from("2026-08-11T10:00:00Z")),
            ..Default::default()
        }),
        end: Some(GcalEventDateTime {
            date_time: Some(String::from("2026-08-11T11:00:00Z")),
            ..Default::default()
        }),
        ..Default::default()
    };

    client
        .event_update("primary", "ev1", &event, &Default::default())
        .expect("event update");
    client
        .event_import("primary", &event, &Default::default())
        .expect("event import");
    client
        .settings_watch(&channel(), &Default::default())
        .expect("settings watch");
    client
        .calendar_transfer_ownership("cal-1", "new@example.org", true)
        .expect("calendar transfer ownership");

    let requests = written(&mut client);
    assert!(requests.contains("PUT /calendar/v3/calendars/primary/events/ev1 HTTP/1.1"));
    assert!(requests.contains("POST /calendar/v3/calendars/primary/events/import HTTP/1.1"));
    assert!(requests.contains("POST /calendar/v3/users/me/settings/watch HTTP/1.1"));
    assert!(
        requests.contains(
            "POST /calendar/v3/calendars/cal-1/transferOwnership?newDataOwner=new%40example.org&useAdminAccess=true"
        )
    );
}

#[test]
fn surfaces_an_api_error() {
    let body = r#"{"error":{"code":404,"message":"Not Found"}}"#;
    let mut client = client(vec![json_response("404 Not Found", body)]);

    let Err(err) = client.calendar_get("missing") else {
        panic!("expected an API error");
    };

    assert!(
        matches!(&err, GcalClientStdError::Send(send) if send.status() == Some(404)),
        "got: {err}"
    );
}

#[test]
fn surfaces_a_stream_failure() {
    let mut client = GcalClientStd::new(FailingStream, "fake-token");

    let Err(err) = client.colors_get() else {
        panic!("expected an I/O error");
    };

    assert!(matches!(err, GcalClientStdError::Io(_)), "got: {err}");
}

#[test]
fn swaps_its_stream() {
    let mut client = GcalClientStd::new(FailingStream, "fake-token");
    client.set_stream(ScriptedStream::new([json_response("200 OK", "{}")]));

    client.colors_get().expect("colors get after the swap");
}

#[test]
fn keeps_the_token_out_of_its_debug_output() {
    let client = client(vec![]);
    let debug = format!("{client:?}");

    assert!(debug.contains("GcalClientStd"), "got: {debug}");
    assert!(!debug.contains("fake-token"), "got: {debug}");
}
