mod common;

use io_gcal::v3::{
    query::to_query_pairs,
    rest::{
        acl::{
            GcalAccessRole, GcalAclRule, GcalAclScope, GcalAclScopeType, insert::GcalAclRuleInsert,
            list::GcalAclList, list::GcalAclListParams,
        },
        calendar_list::{
            GcalCalendarListEntry, insert::GcalCalendarListEntryInsert, list::GcalCalendarListList,
            list::GcalCalendarListListParams,
        },
        calendars::{GcalCalendar, clear::GcalCalendarClear, insert::GcalCalendarInsert},
        channels::{GcalChannel, GcalChannelType, stop::GcalChannelStop},
        colors::get::GcalColorsGet,
        events::{
            GcalEvent, GcalEventDateTime, GcalEventStatus, GcalEventType, GcalSendUpdates,
            delete::GcalEventDelete, get::GcalEventGet, insert::GcalEventInsert,
            insert::GcalEventInsertParams, list::GcalEventsList, list::GcalEventsListParams,
            list::GcalEventsOrderBy, r#move::GcalEventMove, quick_add::GcalEventQuickAdd,
        },
        freebusy::{GcalFreeBusyRequest, GcalFreeBusyRequestItem, query::GcalFreeBusyQuery},
        settings::get::GcalSettingGet,
    },
    send::{GcalSendError, parse_api_error},
};
use io_http::rfc6750::bearer::HttpAuthBearer;

use common::{empty_response, json_response, run};

fn auth() -> HttpAuthBearer {
    HttpAuthBearer::new("fake-token")
}

#[test]
fn lists_events() {
    let response = json_response(
        "HTTP/1.1 200 OK",
        r#"{"kind":"calendar#events","accessRole":"owner","nextSyncToken":"sync-1","items":[{"id":"ev1","status":"confirmed","summary":"Standup","start":{"dateTime":"2026-08-10T09:00:00+02:00"},"end":{"dateTime":"2026-08-10T09:15:00+02:00"},"eventType":"default"}]}"#,
    );
    let params = GcalEventsListParams {
        single_events: true,
        max_results: Some(50),
        order_by: Some(GcalEventsOrderBy::StartTime),
        time_min: Some("2026-08-01T00:00:00Z"),
        event_types: &[GcalEventType::Default, GcalEventType::OutOfOffice],
        ical_uid: Some("abc@google.com"),
        ..Default::default()
    };
    let mut coroutine = GcalEventsList::new(&auth(), "primary", &params).unwrap();
    let (ret, written) = run(&mut coroutine, &response);
    let out = ret.unwrap();

    assert_eq!(out.response.items.len(), 1);
    assert_eq!(out.response.items[0].id.as_deref(), Some("ev1"));
    assert_eq!(
        out.response.items[0].status,
        Some(GcalEventStatus::Confirmed)
    );
    assert_eq!(out.response.access_role, Some(GcalAccessRole::Owner));
    assert_eq!(out.response.next_sync_token.as_deref(), Some("sync-1"));

    let request = String::from_utf8_lossy(&written);
    assert!(
        request.starts_with("GET /calendar/v3/calendars/primary/events?"),
        "got: {request}"
    );
    assert!(request.contains("singleEvents=true"), "got: {request}");
    assert!(request.contains("orderBy=startTime"), "got: {request}");
    assert!(
        request.contains("eventTypes=default&eventTypes=outOfOffice"),
        "got: {request}"
    );
    assert!(
        request.contains("iCalUID=abc%40google.com"),
        "got: {request}"
    );
    assert!(
        request.contains("Authorization: Bearer fake-token"),
        "got: {request}"
    );
}

#[test]
fn omits_unset_list_params() {
    let params = GcalEventsListParams::default();
    let pairs = to_query_pairs(&params);
    assert!(pairs.is_empty(), "got: {pairs:?}");
}

#[test]
fn gets_event_with_scalar_params() {
    let response = json_response(
        "HTTP/1.1 200 OK",
        r#"{"id":"ev1","iCalUID":"uid-1@google.com","summary":"Lunch","organizer":{"email":"me@example.org","self":true},"extendedProperties":{"private":{"k":"v"}}}"#,
    );
    let mut coroutine = GcalEventGet::new(
        &auth(),
        "me@example.org",
        "ev1",
        Some(5),
        Some("Europe/Paris"),
    )
    .unwrap();
    let (ret, written) = run(&mut coroutine, &response);
    let out = ret.unwrap();

    assert_eq!(out.response.ical_uid.as_deref(), Some("uid-1@google.com"));
    assert_eq!(
        out.response
            .organizer
            .and_then(|organizer| organizer.is_self),
        Some(true)
    );
    assert_eq!(
        out.response
            .extended_properties
            .and_then(|properties| properties.private.get("k").cloned()),
        Some(String::from("v"))
    );

    let request = String::from_utf8_lossy(&written);
    assert!(
        request.starts_with("GET /calendar/v3/calendars/me@example.org/events/ev1?"),
        "got: {request}"
    );
    assert!(request.contains("maxAttendees=5"), "got: {request}");
    assert!(
        request.contains("timeZone=Europe%2FParis"),
        "got: {request}"
    );
}

#[test]
fn inserts_event_with_params() {
    let response = json_response("HTTP/1.1 200 OK", r#"{"id":"ev2"}"#);
    let event = GcalEvent {
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
    let params = GcalEventInsertParams {
        send_updates: Some(GcalSendUpdates::All),
        conference_data_version: Some(1),
        supports_attachments: true,
        ..Default::default()
    };
    let mut coroutine = GcalEventInsert::new(&auth(), "primary", &event, &params).unwrap();
    let (ret, written) = run(&mut coroutine, &response);

    assert_eq!(ret.unwrap().response.id.as_deref(), Some("ev2"));

    let request = String::from_utf8_lossy(&written);
    assert!(
        request.starts_with("POST /calendar/v3/calendars/primary/events?"),
        "got: {request}"
    );
    assert!(request.contains("sendUpdates=all"), "got: {request}");
    assert!(
        request.contains("conferenceDataVersion=1"),
        "got: {request}"
    );
    assert!(
        request.contains("supportsAttachments=true"),
        "got: {request}"
    );
    assert!(request.contains(r#""summary":"Review""#), "got: {request}");
    // NOTE: the unset fields must not reach the wire, or a write would
    // clear them server-side.
    assert!(!request.contains(r#""location""#), "got: {request}");
}

#[test]
fn rejects_event_without_start_and_end() {
    let event = GcalEvent {
        summary: Some(String::from("Review")),
        ..Default::default()
    };
    let Err(err) = GcalEventInsert::new(&auth(), "primary", &event, &Default::default()) else {
        panic!("expected an invalid request error");
    };

    assert!(
        matches!(err, GcalSendError::InvalidRequest(ref message) if message.contains("start")),
        "got: {err}"
    );
}

#[test]
fn rejects_calendar_without_summary() {
    let Err(err) = GcalCalendarInsert::new(&auth(), &GcalCalendar::default()) else {
        panic!("expected an invalid request error");
    };

    assert!(
        matches!(err, GcalSendError::InvalidRequest(ref message) if message.contains("summary")),
        "got: {err}"
    );
}

#[test]
fn rejects_calendar_list_entry_without_id() {
    let Err(err) =
        GcalCalendarListEntryInsert::new(&auth(), &GcalCalendarListEntry::default(), None)
    else {
        panic!("expected an invalid request error");
    };

    assert!(
        matches!(err, GcalSendError::InvalidRequest(ref message) if message.contains("id")),
        "got: {err}"
    );
}

#[test]
fn rejects_free_busy_query_without_calendar() {
    let Err(err) = GcalFreeBusyQuery::new(&auth(), &GcalFreeBusyRequest::default()) else {
        panic!("expected an invalid request error");
    };

    assert!(
        matches!(err, GcalSendError::InvalidRequest(ref message) if message.contains("calendar")),
        "got: {err}"
    );
}

#[test]
fn deletes_event_and_accepts_empty_body() {
    let response = empty_response("HTTP/1.1 204 No Content");
    let mut coroutine = GcalEventDelete::new(
        &auth(),
        "primary",
        "ev1",
        Some(GcalSendUpdates::ExternalOnly),
    )
    .unwrap();
    let (ret, written) = run(&mut coroutine, &response);

    ret.unwrap();

    let request = String::from_utf8_lossy(&written);
    assert!(
        request.starts_with("DELETE /calendar/v3/calendars/primary/events/ev1?"),
        "got: {request}"
    );
    assert!(
        request.contains("sendUpdates=externalOnly"),
        "got: {request}"
    );
}

#[test]
fn moves_event_to_another_calendar() {
    let response = json_response("HTTP/1.1 200 OK", r#"{"id":"ev1"}"#);
    let mut coroutine =
        GcalEventMove::new(&auth(), "primary", "ev1", "other@example.org", None).unwrap();
    let (ret, written) = run(&mut coroutine, &response);

    ret.unwrap();

    let request = String::from_utf8_lossy(&written);
    assert!(
        request.starts_with("POST /calendar/v3/calendars/primary/events/ev1/move?"),
        "got: {request}"
    );
    assert!(
        request.contains("destination=other%40example.org"),
        "got: {request}"
    );
    assert!(!request.contains("sendUpdates"), "got: {request}");
}

#[test]
fn quick_adds_event_from_text() {
    let response = json_response("HTTP/1.1 200 OK", r#"{"id":"ev3"}"#);
    let mut coroutine =
        GcalEventQuickAdd::new(&auth(), "primary", "Lunch tomorrow 12pm", None).unwrap();
    let (ret, written) = run(&mut coroutine, &response);

    ret.unwrap();

    let request = String::from_utf8_lossy(&written);
    assert!(
        request.contains("text=Lunch+tomorrow+12pm"),
        "got: {request}"
    );
}

#[test]
fn rejects_empty_quick_add_text() {
    let Err(err) = GcalEventQuickAdd::new(&auth(), "primary", "   ", None) else {
        panic!("expected an invalid request error");
    };

    assert!(
        matches!(err, GcalSendError::InvalidRequest(ref message) if message.contains("empty")),
        "got: {err}"
    );
}

#[test]
fn lists_acl_rules() {
    let response = json_response(
        "HTTP/1.1 200 OK",
        r#"{"kind":"calendar#acl","items":[{"id":"user:jane@example.org","role":"writer","scope":{"type":"user","value":"jane@example.org"}}]}"#,
    );
    let params = GcalAclListParams {
        show_deleted: true,
        max_results: Some(10),
        ..Default::default()
    };
    let mut coroutine = GcalAclList::new(&auth(), "primary", &params).unwrap();
    let (ret, written) = run(&mut coroutine, &response);
    let out = ret.unwrap();

    assert_eq!(out.response.items.len(), 1);
    assert_eq!(out.response.items[0].role, Some(GcalAccessRole::Writer));
    assert_eq!(
        out.response.items[0]
            .scope
            .as_ref()
            .and_then(|scope| scope.scope_type),
        Some(GcalAclScopeType::User)
    );

    let request = String::from_utf8_lossy(&written);
    assert!(
        request.starts_with("GET /calendar/v3/calendars/primary/acl?"),
        "got: {request}"
    );
    assert!(request.contains("showDeleted=true"), "got: {request}");
}

#[test]
fn inserts_acl_rule_with_notifications() {
    let response = json_response("HTTP/1.1 200 OK", r#"{"id":"user:jane@example.org"}"#);
    let rule = GcalAclRule {
        role: Some(GcalAccessRole::Reader),
        scope: Some(GcalAclScope {
            scope_type: Some(GcalAclScopeType::User),
            value: Some(String::from("jane@example.org")),
        }),
        ..Default::default()
    };
    let mut coroutine = GcalAclRuleInsert::new(&auth(), "primary", &rule, Some(false)).unwrap();
    let (ret, written) = run(&mut coroutine, &response);

    ret.unwrap();

    let request = String::from_utf8_lossy(&written);
    assert!(
        request.contains("sendNotifications=false"),
        "got: {request}"
    );
    assert!(request.contains(r#""role":"reader""#), "got: {request}");
    assert!(request.contains(r#""type":"user""#), "got: {request}");
}

#[test]
fn lists_calendar_list_with_min_access_role() {
    let response = json_response(
        "HTTP/1.1 200 OK",
        r#"{"items":[{"id":"primary","accessRole":"owner","primary":true}]}"#,
    );
    let params = GcalCalendarListListParams {
        min_access_role: Some(GcalAccessRole::Writer),
        show_hidden: true,
        ..Default::default()
    };
    let mut coroutine = GcalCalendarListList::new(&auth(), &params).unwrap();
    let (ret, written) = run(&mut coroutine, &response);
    let out = ret.unwrap();

    assert_eq!(out.response.items[0].primary, Some(true));

    let request = String::from_utf8_lossy(&written);
    assert!(
        request.starts_with("GET /calendar/v3/users/me/calendarList?"),
        "got: {request}"
    );
    assert!(request.contains("minAccessRole=writer"), "got: {request}");
    assert!(request.contains("showHidden=true"), "got: {request}");
}

#[test]
fn gets_colors() {
    let response = json_response(
        "HTTP/1.1 200 OK",
        r##"{"kind":"calendar#colors","event":{"1":{"background":"#a4bdfc","foreground":"#1d1d1d"}},"calendar":{}}"##,
    );
    let mut coroutine = GcalColorsGet::new(&auth()).unwrap();
    let (ret, written) = run(&mut coroutine, &response);
    let out = ret.unwrap();

    assert_eq!(
        out.response.event["1"].background.as_deref(),
        Some("#a4bdfc")
    );

    let request = String::from_utf8_lossy(&written);
    assert!(
        request.starts_with("GET /calendar/v3/colors "),
        "got: {request}"
    );
}

#[test]
fn queries_free_busy() {
    let response = json_response(
        "HTTP/1.1 200 OK",
        r#"{"timeMin":"2026-08-01T00:00:00Z","calendars":{"primary":{"busy":[{"start":"2026-08-01T09:00:00Z","end":"2026-08-01T10:00:00Z"}]}}}"#,
    );
    let request_body = GcalFreeBusyRequest {
        time_min: Some(String::from("2026-08-01T00:00:00Z")),
        time_max: Some(String::from("2026-08-02T00:00:00Z")),
        items: vec![GcalFreeBusyRequestItem {
            id: Some(String::from("primary")),
        }],
        ..Default::default()
    };
    let mut coroutine = GcalFreeBusyQuery::new(&auth(), &request_body).unwrap();
    let (ret, written) = run(&mut coroutine, &response);
    let out = ret.unwrap();

    assert_eq!(out.response.calendars["primary"].busy.len(), 1);

    let request = String::from_utf8_lossy(&written);
    assert!(
        request.starts_with("POST /calendar/v3/freeBusy "),
        "got: {request}"
    );
    assert!(request.contains(r#""id":"primary""#), "got: {request}");
}

#[test]
fn clears_calendar() {
    let response = empty_response("HTTP/1.1 204 No Content");
    let mut coroutine = GcalCalendarClear::new(&auth(), "primary").unwrap();
    let (ret, written) = run(&mut coroutine, &response);

    ret.unwrap();

    let request = String::from_utf8_lossy(&written);
    assert!(
        request.starts_with("POST /calendar/v3/calendars/primary/clear "),
        "got: {request}"
    );
}

#[test]
fn stops_channel() {
    let response = empty_response("HTTP/1.1 204 No Content");
    let channel = GcalChannel {
        id: Some(String::from("chan-1")),
        resource_id: Some(String::from("res-1")),
        channel_type: Some(GcalChannelType::WebHook),
        ..Default::default()
    };
    let mut coroutine = GcalChannelStop::new(&auth(), &channel).unwrap();
    let (ret, written) = run(&mut coroutine, &response);

    ret.unwrap();

    let request = String::from_utf8_lossy(&written);
    assert!(
        request.starts_with("POST /calendar/v3/channels/stop "),
        "got: {request}"
    );
    assert!(request.contains(r#""type":"web_hook""#), "got: {request}");
}

#[test]
fn accepts_the_webhook_channel_type_alias() {
    let channel: GcalChannel = serde_json::from_str(r#"{"type":"webhook"}"#).unwrap();

    assert_eq!(channel.channel_type, Some(GcalChannelType::WebHook));
}

#[test]
fn gets_setting() {
    let response = json_response(
        "HTTP/1.1 200 OK",
        r#"{"kind":"calendar#setting","id":"timezone","value":"Europe/Paris"}"#,
    );
    let mut coroutine = GcalSettingGet::new(&auth(), "timezone").unwrap();
    let (ret, written) = run(&mut coroutine, &response);
    let out = ret.unwrap();

    assert_eq!(out.response.value.as_deref(), Some("Europe/Paris"));

    let request = String::from_utf8_lossy(&written);
    assert!(
        request.starts_with("GET /calendar/v3/users/me/settings/timezone "),
        "got: {request}"
    );
}

#[test]
fn surfaces_api_errors() {
    let response = json_response(
        "HTTP/1.1 404 Not Found",
        r#"{"error":{"code":404,"message":"Not Found","errors":[]}}"#,
    );
    let mut coroutine = GcalEventGet::new(&auth(), "primary", "missing", None, None).unwrap();
    let (ret, _) = run(&mut coroutine, &response);
    let err = ret.unwrap_err();

    assert_eq!(err.status(), Some(404));
    assert!(!err.is_retryable());
    assert!(!err.is_sync_token_expired());
}

#[test]
fn recognises_an_expired_sync_token() {
    let response = json_response(
        "HTTP/1.1 410 Gone",
        r#"{"error":{"code":410,"message":"Sync token is no longer valid, a full sync is required."}}"#,
    );
    let params = GcalEventsListParams {
        sync_token: Some("stale"),
        ..Default::default()
    };
    let mut coroutine = GcalEventsList::new(&auth(), "primary", &params).unwrap();
    let (ret, _) = run(&mut coroutine, &response);
    let err = ret.unwrap_err();

    assert!(err.is_sync_token_expired());
}

#[test]
fn parses_error_bodies() {
    let (status, message) =
        parse_api_error(400, br#"{"error":{"code":403,"message":"Forbidden"}}"#);
    assert_eq!(status, 403);
    assert_eq!(message, "Forbidden");

    let (status, message) = parse_api_error(500, b"  boom  ");
    assert_eq!(status, 500);
    assert_eq!(message, "boom");

    let (status, message) = parse_api_error(503, b"");
    assert_eq!(status, 503);
    assert_eq!(message, "unknown Calendar API error");
}

#[test]
fn round_trips_an_event() {
    let json = r#"{"id":"ev1","status":"cancelled","iCalUID":"uid@google.com","eventType":"workingLocation","workingLocationProperties":{"type":"officeLocation","officeLocation":{"label":"Paris"}},"attendees":[{"email":"jane@example.org","responseStatus":"accepted","self":true}],"reminders":{"useDefault":false,"overrides":[{"method":"popup","minutes":10}]},"recurrence":["RRULE:FREQ=WEEKLY"]}"#;

    let event: GcalEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.status, Some(GcalEventStatus::Cancelled));
    assert_eq!(event.event_type, Some(GcalEventType::WorkingLocation));

    let reserialized = serde_json::to_string(&event).unwrap();
    let reparsed: GcalEvent = serde_json::from_str(&reserialized).unwrap();
    assert_eq!(event, reparsed);
    assert!(reserialized.contains(r#""iCalUID":"uid@google.com""#));
    assert!(reserialized.contains(r#""self":true"#));
    assert!(reserialized.contains(r#""type":"officeLocation""#));
}
