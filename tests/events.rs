//! Offline coverage of the events (`events`): the eleven methods,
//! their query parameters, the validation they apply and the whole
//! event representation.

mod common;

use common::*;
use io_gcal::v3::{
    rest::{
        channels::{GcalChannel, GcalChannelType},
        events::{
            GcalConferenceData, GcalConferenceRequestStatusCode, GcalConferenceSolution,
            GcalConferenceSolutionKey, GcalEntryPoint, GcalEntryPointType, GcalEvent,
            GcalEventAttendee, GcalEventAttendeeAsyncOperation, GcalEventAttendeeResponseStatus,
            GcalEventAutoDeclineMode, GcalEventBirthdayType, GcalEventChatStatus,
            GcalEventDateTime, GcalEventExtendedProperties, GcalEventReminder,
            GcalEventReminderMethod, GcalEventReminders, GcalEventSource, GcalEventStatus,
            GcalEventTransparency, GcalEventType, GcalEventVisibility,
            GcalEventWorkingLocationOffice, GcalEventWorkingLocationProperties,
            GcalEventWorkingLocationType, GcalEvents, GcalSendUpdates, delete::GcalEventDelete,
            get::GcalEventGet, import::GcalEventImport, import::GcalEventImportParams,
            insert::GcalEventInsert, insert::GcalEventInsertParams, instances::GcalEventInstances,
            instances::GcalEventInstancesParams, list::GcalEventsList, list::GcalEventsListParams,
            list::GcalEventsOrderBy, r#move::GcalEventMove, patch::GcalEventPatch,
            quick_add::GcalEventQuickAdd, update::GcalEventUpdate, update::GcalEventUpdateParams,
            watch::GcalEventsWatch,
        },
    },
    send::GcalSendError,
};

const EVENT: &str = r#"{"kind":"calendar#event","etag":"\"tag-1\"","id":"ev1","status":"confirmed","htmlLink":"https://www.google.com/calendar/event?eid=x","created":"2026-08-01T10:00:00.000Z","updated":"2026-08-02T10:00:00.000Z","summary":"Standup","start":{"dateTime":"2026-08-10T09:00:00+02:00","timeZone":"Europe/Paris"},"end":{"dateTime":"2026-08-10T09:15:00+02:00"},"iCalUID":"ev1@google.com","sequence":0,"eventType":"default","organizer":{"email":"jane@example.org","self":true},"reminders":{"useDefault":true}}"#;

fn event() -> GcalEvent {
    GcalEvent {
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
    }
}

// --- list ----------------------------------------------------------------

#[test]
fn lists_the_events_of_a_calendar() {
    let body = format!(
        r#"{{"kind":"calendar#events","summary":"Jane","accessRole":"owner","defaultReminders":[{{"method":"popup","minutes":10}}],"items":[{EVENT}],"nextPageToken":"p2"}}"#
    );
    let private = [String::from("kind=io-gcal")];
    let shared = [String::from("team=core")];
    let params = GcalEventsListParams {
        q: Some("standup"),
        event_types: &[GcalEventType::Default, GcalEventType::OutOfOffice],
        ical_uid: Some("ev1@google.com"),
        time_min: Some("2026-08-01T00:00:00Z"),
        time_max: Some("2026-09-01T00:00:00Z"),
        updated_min: Some("2026-07-01T00:00:00Z"),
        time_zone: Some("Europe/Paris"),
        max_attendees: Some(5),
        max_results: Some(50),
        order_by: Some(GcalEventsOrderBy::StartTime),
        page_token: Some("p1"),
        private_extended_property: &private,
        shared_extended_property: &shared,
        single_events: true,
        show_deleted: true,
        show_hidden_invitations: true,
        sync_token: None,
    };
    let mut coroutine = GcalEventsList::new(&auth(), "primary", &params).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", &body));
    let out = ret.unwrap();

    assert_eq!(out.response.items.len(), 1);
    assert_eq!(out.response.items[0].summary.as_deref(), Some("Standup"));
    assert_eq!(out.response.next_page_token.as_deref(), Some("p2"));
    assert_eq!(out.response.default_reminders.len(), 1);

    assert!(
        request_line(&request).starts_with("GET /calendar/v3/calendars/primary/events?"),
        "got: {request}"
    );
    assert_query(&request, "q=standup");
    assert_query(&request, "eventTypes=default&eventTypes=outOfOffice");
    assert_query(&request, "iCalUID=ev1%40google.com");
    assert_query(&request, "timeMin=2026-08-01T00%3A00%3A00Z");
    assert_query(&request, "timeMax=2026-09-01T00%3A00%3A00Z");
    assert_query(&request, "updatedMin=2026-07-01T00%3A00%3A00Z");
    assert_query(&request, "timeZone=Europe%2FParis");
    assert_query(&request, "maxAttendees=5");
    assert_query(&request, "maxResults=50");
    assert_query(&request, "orderBy=startTime");
    assert_query(&request, "pageToken=p1");
    assert_query(&request, "privateExtendedProperty=kind%3Dio-gcal");
    assert_query(&request, "sharedExtendedProperty=team%3Dcore");
    assert_query(&request, "singleEvents=true");
    assert_query(&request, "showDeleted=true");
    assert_query(&request, "showHiddenInvitations=true");
}

#[test]
fn lists_the_events_without_any_parameter() {
    let mut coroutine = GcalEventsList::new(&auth(), "primary", &Default::default()).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", "{}"));
    let out = ret.unwrap();

    assert!(out.response.items.is_empty());

    assert_eq!(
        request_line(&request),
        "GET /calendar/v3/calendars/primary/events"
    );
}

#[test]
fn lists_the_events_incrementally() {
    let body = r#"{"items":[{"id":"ev1","status":"cancelled","recurringEventId":"series-1","originalStartTime":{"dateTime":"2026-08-10T09:00:00+02:00"}}],"nextSyncToken":"s2"}"#;
    let params = GcalEventsListParams {
        sync_token: Some("s1"),
        single_events: true,
        ..Default::default()
    };
    let mut coroutine = GcalEventsList::new(&auth(), "primary", &params).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", body));
    let out = ret.unwrap();

    let cancelled = &out.response.items[0];
    assert_eq!(cancelled.status, Some(GcalEventStatus::Cancelled));
    assert_eq!(cancelled.recurring_event_id.as_deref(), Some("series-1"));
    assert!(cancelled.original_start_time.is_some());
    assert_eq!(out.response.next_sync_token.as_deref(), Some("s2"));

    assert_query(&request, "syncToken=s1");
    // NOTE: the API rejects an explicit `showDeleted=false` next to a
    // sync token, so an unset flag must stay off the wire.
    assert_no_query(&request, "showDeleted");
}

#[test]
fn reports_an_expired_sync_token() {
    let body = r#"{"error":{"code":410,"message":"Sync token is no longer valid, a full sync is required."}}"#;
    let params = GcalEventsListParams {
        sync_token: Some("stale"),
        ..Default::default()
    };
    let mut coroutine = GcalEventsList::new(&auth(), "primary", &params).unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &json_response("410 Gone", body));
    let Err(err) = ret else {
        panic!("expected an API error");
    };

    assert!(err.is_sync_token_expired());
    assert!(!err.is_retryable());
}

#[test]
fn never_sends_the_deprecated_parameters() {
    let params = GcalEventsListParams {
        max_attendees: Some(1),
        ..Default::default()
    };
    let mut coroutine = GcalEventsList::new(&auth(), "primary", &params).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", "{}"));

    ret.unwrap();

    assert_no_query(&request, "alwaysIncludeEmail");
    assert_no_query(&request, "sendNotifications");
}

// --- get -----------------------------------------------------------------

#[test]
fn gets_an_event() {
    let mut coroutine = GcalEventGet::new(&auth(), "primary", "ev1", None, None).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", EVENT));
    let out = ret.unwrap();

    assert_eq!(out.response.id.as_deref(), Some("ev1"));
    assert_eq!(out.response.ical_uid.as_deref(), Some("ev1@google.com"));
    assert_eq!(out.response.status, Some(GcalEventStatus::Confirmed));
    assert_eq!(
        out.response
            .start
            .as_ref()
            .and_then(|start| start.time_zone.as_deref()),
        Some("Europe/Paris")
    );
    assert_eq!(
        out.response
            .organizer
            .as_ref()
            .and_then(|organizer| organizer.is_self),
        Some(true)
    );
    assert_eq!(
        out.response
            .reminders
            .as_ref()
            .and_then(|reminders| reminders.use_default),
        Some(true)
    );

    assert_eq!(
        request_line(&request),
        "GET /calendar/v3/calendars/primary/events/ev1"
    );
}

#[test]
fn gets_an_event_with_scalar_parameters() {
    let mut coroutine = GcalEventGet::new(
        &auth(),
        "team@group.calendar.google.com",
        "ev1",
        Some(5),
        Some("Europe/Paris"),
    )
    .unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", EVENT));

    ret.unwrap();

    assert!(
        request_line(&request)
            .starts_with("GET /calendar/v3/calendars/team@group.calendar.google.com/events/ev1?"),
        "got: {request}"
    );
    assert_query(&request, "maxAttendees=5");
    assert_query(&request, "timeZone=Europe%2FParis");
}

// --- insert, update, patch -----------------------------------------------

#[test]
fn inserts_an_event() {
    let params = GcalEventInsertParams {
        send_updates: Some(GcalSendUpdates::All),
        conference_data_version: Some(1),
        event_label_version: Some(1),
        max_attendees: Some(10),
        supports_attachments: true,
    };
    let mut coroutine = GcalEventInsert::new(&auth(), "primary", &event(), &params).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", EVENT));

    ret.unwrap();

    assert!(
        request_line(&request).starts_with("POST /calendar/v3/calendars/primary/events?"),
        "got: {request}"
    );
    assert_query(&request, "sendUpdates=all");
    assert_query(&request, "conferenceDataVersion=1");
    assert_query(&request, "eventLabelVersion=1");
    assert_query(&request, "maxAttendees=10");
    assert_query(&request, "supportsAttachments=true");
    assert_eq!(
        request_body(&request),
        r#"{"summary":"Review","start":{"dateTime":"2026-08-11T10:00:00Z"},"end":{"dateTime":"2026-08-11T11:00:00Z"}}"#
    );
}

#[test]
fn inserts_an_event_without_any_parameter() {
    let mut coroutine =
        GcalEventInsert::new(&auth(), "primary", &event(), &Default::default()).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", EVENT));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "POST /calendar/v3/calendars/primary/events"
    );
}

#[test]
fn refuses_an_event_without_a_start_or_an_end() {
    let cases = [
        GcalEvent::default(),
        GcalEvent {
            start: event().start,
            ..Default::default()
        },
        GcalEvent {
            end: event().end,
            ..Default::default()
        },
    ];

    for event in cases {
        let Err(err) = GcalEventInsert::new(&auth(), "primary", &event, &Default::default()) else {
            panic!("expected an invalid request error");
        };

        assert!(
            matches!(err, GcalSendError::InvalidRequest(ref message) if message.contains("start")),
            "got: {err}"
        );
    }
}

#[test]
fn refuses_a_timed_series_without_a_time_zone() {
    // NOTE: the live API answers this one with "Missing time zone
    // definition for start time", since the recurrence is expanded in
    // the time zone of the start and a UTC offset does not name one.
    let series = GcalEvent {
        recurrence: vec![String::from("RRULE:FREQ=WEEKLY;COUNT=3")],
        ..event()
    };

    let Err(err) = GcalEventInsert::new(&auth(), "primary", &series, &Default::default()) else {
        panic!("expected an invalid request error");
    };
    assert!(
        matches!(err, GcalSendError::InvalidRequest(ref message) if message.contains("time zone")),
        "got: {err}"
    );

    let imported = GcalEvent {
        ical_uid: Some(String::from("imported@example.org")),
        ..series
    };
    let Err(err) = GcalEventImport::new(&auth(), "primary", &imported, &Default::default()) else {
        panic!("expected an invalid request error");
    };
    assert!(
        matches!(err, GcalSendError::InvalidRequest(ref message) if message.contains("time zone")),
        "got: {err}"
    );
}

#[test]
fn accepts_a_series_anchored_in_a_time_zone() {
    let anchored = |date_time: &str| GcalEventDateTime {
        date_time: Some(String::from(date_time)),
        time_zone: Some(String::from("Europe/Paris")),
        ..Default::default()
    };
    let series = GcalEvent {
        start: Some(anchored("2030-02-04T10:00:00+01:00")),
        end: Some(anchored("2030-02-04T10:30:00+01:00")),
        recurrence: vec![String::from("RRULE:FREQ=WEEKLY;COUNT=3")],
        ..event()
    };

    let mut coroutine =
        GcalEventInsert::new(&auth(), "primary", &series, &Default::default()).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", EVENT));

    ret.unwrap();

    assert!(
        request_body(&request).contains(r#""timeZone":"Europe/Paris""#),
        "got: {request}"
    );
}

#[test]
fn accepts_an_all_day_series_without_a_time_zone() {
    // NOTE: an all-day series is dated rather than timed, so there is
    // no time to anchor and the API asks for no time zone.
    let dated = |date: &str| GcalEventDateTime {
        date: Some(String::from(date)),
        ..Default::default()
    };
    let series = GcalEvent {
        start: Some(dated("2030-02-04")),
        end: Some(dated("2030-02-05")),
        recurrence: vec![String::from("RRULE:FREQ=WEEKLY;COUNT=3")],
        ..event()
    };

    let mut coroutine =
        GcalEventInsert::new(&auth(), "primary", &series, &Default::default()).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", EVENT));

    ret.unwrap();

    assert!(
        request_body(&request).contains(r#""date":"2030-02-04""#),
        "got: {request}"
    );
}

#[test]
fn tells_an_unanchored_boundary_from_an_anchored_one() {
    let timed = GcalEventDateTime {
        date_time: Some(String::from("2030-02-04T10:00:00+01:00")),
        ..Default::default()
    };
    assert!(timed.is_timed_without_time_zone());

    let anchored = GcalEventDateTime {
        time_zone: Some(String::from("Europe/Paris")),
        ..timed.clone()
    };
    assert!(!anchored.is_timed_without_time_zone());

    let dated = GcalEventDateTime {
        date: Some(String::from("2030-02-04")),
        ..Default::default()
    };
    assert!(!dated.is_timed_without_time_zone());
}

#[test]
fn updates_an_event() {
    let params = GcalEventUpdateParams {
        send_updates: Some(GcalSendUpdates::ExternalOnly),
        ..Default::default()
    };
    let mut coroutine =
        GcalEventUpdate::new(&auth(), "primary", "ev1", &event(), &params, None).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", EVENT));

    ret.unwrap();

    assert!(
        request_line(&request).starts_with("PUT /calendar/v3/calendars/primary/events/ev1?"),
        "got: {request}"
    );
    assert_query(&request, "sendUpdates=externalOnly");
}

#[test]
fn patches_an_event() {
    let patch = GcalEvent {
        location: Some(String::from("Somewhere")),
        ..Default::default()
    };
    let mut coroutine =
        GcalEventPatch::new(&auth(), "primary", "ev1", &patch, &Default::default(), None).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", EVENT));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "PATCH /calendar/v3/calendars/primary/events/ev1"
    );
    // NOTE: a patch must not carry the fields it does not change, or
    // the server clears them.
    assert_eq!(request_body(&request), r#"{"location":"Somewhere"}"#);
}

// --- delete, import, instances, move, quick add, watch -------------------

#[test]
fn deletes_an_event() {
    let mut coroutine = GcalEventDelete::new(&auth(), "primary", "ev1", None, None).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &empty_response("204 No Content"));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "DELETE /calendar/v3/calendars/primary/events/ev1"
    );
}

#[test]
fn guards_a_write_on_an_entity_tag() {
    for (method, request) in [
        ("PUT", {
            let mut coroutine = GcalEventUpdate::new(
                &auth(),
                "primary",
                "ev1",
                &event(),
                &Default::default(),
                Some("\"tag-1\""),
            )
            .unwrap();
            let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", EVENT));
            ret.unwrap();
            request
        }),
        ("PATCH", {
            let mut coroutine = GcalEventPatch::new(
                &auth(),
                "primary",
                "ev1",
                &event(),
                &Default::default(),
                Some("\"tag-1\""),
            )
            .unwrap();
            let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", EVENT));
            ret.unwrap();
            request
        }),
        ("DELETE", {
            let mut coroutine =
                GcalEventDelete::new(&auth(), "primary", "ev1", None, Some("\"tag-1\"")).unwrap();
            let (request, ret) = expect_exchange(&mut coroutine, &empty_response("204 No Content"));
            ret.unwrap();
            request
        }),
    ] {
        assert!(request.starts_with(method), "got: {request}");
        assert!(
            request.contains("If-Match: \"tag-1\""),
            "{method} got: {request}"
        );
    }
}

#[test]
fn leaves_an_unguarded_write_unconditional() {
    let mut coroutine = GcalEventUpdate::new(
        &auth(),
        "primary",
        "ev1",
        &event(),
        &Default::default(),
        None,
    )
    .unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", EVENT));

    ret.unwrap();

    assert!(!request.contains("If-Match"), "got: {request}");
}

#[test]
fn reports_a_stale_entity_tag() {
    let body = r#"{"error":{"code":412,"message":"Precondition Failed"}}"#;
    let mut coroutine = GcalEventUpdate::new(
        &auth(),
        "primary",
        "ev1",
        &event(),
        &Default::default(),
        Some("\"stale\""),
    )
    .unwrap();
    let (_, ret) = expect_exchange(
        &mut coroutine,
        &json_response("412 Precondition Failed", body),
    );
    let Err(err) = ret else {
        panic!("expected an API error");
    };

    assert!(err.is_precondition_failed());
    assert!(!err.is_retryable());
    assert!(!err.is_sync_token_expired());
}

#[test]
fn deletes_an_event_notifying_nobody() {
    let mut coroutine =
        GcalEventDelete::new(&auth(), "primary", "ev1", Some(GcalSendUpdates::None), None).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &empty_response("204 No Content"));

    ret.unwrap();

    assert_query(&request, "sendUpdates=none");
}

#[test]
fn imports_an_event() {
    let event = GcalEvent {
        ical_uid: Some(String::from("imported@example.org")),
        ..event()
    };
    let params = GcalEventImportParams {
        conference_data_version: Some(0),
        event_label_version: Some(1),
        supports_attachments: true,
    };
    let mut coroutine = GcalEventImport::new(&auth(), "primary", &event, &params).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", EVENT));

    ret.unwrap();

    assert!(
        request_line(&request).starts_with("POST /calendar/v3/calendars/primary/events/import?"),
        "got: {request}"
    );
    assert_query(&request, "conferenceDataVersion=0");
    assert_query(&request, "supportsAttachments=true");
    assert!(
        request_body(&request).contains(r#""iCalUID":"imported@example.org""#),
        "got: {request}"
    );
}

#[test]
fn refuses_an_import_without_an_ical_uid() {
    let Err(err) = GcalEventImport::new(&auth(), "primary", &event(), &Default::default()) else {
        panic!("expected an invalid request error");
    };

    assert!(
        matches!(err, GcalSendError::InvalidRequest(ref message) if message.contains("iCalUID")),
        "got: {err}"
    );
}

#[test]
fn refuses_an_import_without_a_start_or_an_end() {
    let event = GcalEvent {
        ical_uid: Some(String::from("imported@example.org")),
        ..Default::default()
    };
    let Err(err) = GcalEventImport::new(&auth(), "primary", &event, &Default::default()) else {
        panic!("expected an invalid request error");
    };

    assert!(
        matches!(err, GcalSendError::InvalidRequest(ref message) if message.contains("start")),
        "got: {err}"
    );
}

#[test]
fn lists_the_instances_of_a_series() {
    let body = format!(r#"{{"items":[{EVENT}],"nextPageToken":"p2"}}"#);
    let params = GcalEventInstancesParams {
        time_min: Some("2026-08-01T00:00:00Z"),
        time_max: Some("2026-09-01T00:00:00Z"),
        original_start: Some("2026-08-10T09:00:00+02:00"),
        time_zone: Some("Europe/Paris"),
        max_attendees: Some(5),
        max_results: Some(10),
        page_token: Some("p1"),
        show_deleted: true,
    };
    let mut coroutine = GcalEventInstances::new(&auth(), "primary", "series-1", &params).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", &body));
    let out = ret.unwrap();

    assert_eq!(out.response.items.len(), 1);

    assert!(
        request_line(&request)
            .starts_with("GET /calendar/v3/calendars/primary/events/series-1/instances?"),
        "got: {request}"
    );
    assert_query(&request, "originalStart=2026-08-10T09%3A00%3A00%2B02%3A00");
    assert_query(&request, "showDeleted=true");
    assert_query(&request, "maxResults=10");
}

#[test]
fn moves_an_event() {
    let mut coroutine = GcalEventMove::new(
        &auth(),
        "primary",
        "ev1",
        "other@example.org",
        Some(GcalSendUpdates::All),
    )
    .unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", EVENT));

    ret.unwrap();

    assert!(
        request_line(&request).starts_with("POST /calendar/v3/calendars/primary/events/ev1/move?"),
        "got: {request}"
    );
    assert_query(&request, "destination=other%40example.org");
    assert_query(&request, "sendUpdates=all");
    assert!(request.contains("content-length: 0"), "got: {request}");
}

#[test]
fn quick_adds_an_event() {
    let mut coroutine =
        GcalEventQuickAdd::new(&auth(), "primary", "Lunch tomorrow 12pm", None).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", EVENT));

    ret.unwrap();

    assert!(
        request_line(&request).starts_with("POST /calendar/v3/calendars/primary/events/quickAdd?"),
        "got: {request}"
    );
    assert_query(&request, "text=Lunch+tomorrow+12pm");
}

#[test]
fn refuses_a_blank_quick_add_text() {
    let Err(err) = GcalEventQuickAdd::new(&auth(), "primary", "   ", None) else {
        panic!("expected an invalid request error");
    };

    assert!(
        matches!(err, GcalSendError::InvalidRequest(ref message) if message.contains("empty")),
        "got: {err}"
    );
}

#[test]
fn watches_the_events_of_a_calendar() {
    let channel = GcalChannel {
        id: Some(String::from("chan-1")),
        channel_type: Some(GcalChannelType::WebHook),
        address: Some(String::from("https://hook.example.org/gcal")),
        ..Default::default()
    };
    let params = GcalEventsListParams {
        single_events: true,
        ..Default::default()
    };
    let response = json_response("200 OK", r#"{"id":"chan-1","resourceId":"res-1"}"#);
    let mut coroutine = GcalEventsWatch::new(&auth(), "primary", &channel, &params).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &response);
    let out = ret.unwrap();

    assert_eq!(out.response.resource_id.as_deref(), Some("res-1"));

    assert!(
        request_line(&request).starts_with("POST /calendar/v3/calendars/primary/events/watch?"),
        "got: {request}"
    );
    assert_query(&request, "singleEvents=true");
}

// --- representation ------------------------------------------------------

#[test]
fn round_trips_a_whole_event() {
    let event = GcalEvent {
        id: Some(String::from("ev1")),
        status: Some(GcalEventStatus::Tentative),
        summary: Some(String::from("Review")),
        transparency: Some(GcalEventTransparency::Transparent),
        visibility: Some(GcalEventVisibility::Private),
        event_type: Some(GcalEventType::WorkingLocation),
        ical_uid: Some(String::from("ev1@google.com")),
        recurrence: vec![String::from("RRULE:FREQ=WEEKLY;COUNT=3")],
        attendees: vec![GcalEventAttendee {
            email: Some(String::from("jane@example.org")),
            response_status: Some(GcalEventAttendeeResponseStatus::NeedsAction),
            is_self: Some(true),
            additional_guests: Some(2),
            async_operation: Some(GcalEventAttendeeAsyncOperation::InProgress),
            ..Default::default()
        }],
        reminders: Some(GcalEventReminders {
            use_default: Some(false),
            overrides: vec![GcalEventReminder {
                method: Some(GcalEventReminderMethod::Popup),
                minutes: Some(10),
            }],
        }),
        extended_properties: Some(GcalEventExtendedProperties {
            private: [(String::from("kind"), String::from("io-gcal"))]
                .into_iter()
                .collect(),
            shared: Default::default(),
        }),
        source: Some(GcalEventSource {
            url: Some(String::from("https://example.org/")),
            title: Some(String::from("Example")),
        }),
        working_location_properties: Some(GcalEventWorkingLocationProperties {
            working_location_type: Some(GcalEventWorkingLocationType::OfficeLocation),
            office_location: Some(GcalEventWorkingLocationOffice {
                label: Some(String::from("Paris")),
                ..Default::default()
            }),
            ..Default::default()
        }),
        conference_data: Some(GcalConferenceData {
            conference_solution: Some(GcalConferenceSolution {
                key: Some(GcalConferenceSolutionKey {
                    solution_type: Some(
                        io_gcal::v3::rest::calendars::GcalConferenceSolutionType::HangoutsMeet,
                    ),
                }),
                name: Some(String::from("Google Meet")),
                ..Default::default()
            }),
            entry_points: vec![GcalEntryPoint {
                entry_point_type: Some(GcalEntryPointType::Video),
                uri: Some(String::from("https://meet.google.com/aaa-bbbb-ccc")),
                entry_point_features: vec![String::from("toll")],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..event()
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(
        json.contains(r#""iCalUID":"ev1@google.com""#),
        "got: {json}"
    );
    assert!(json.contains(r#""self":true"#), "got: {json}");
    assert!(json.contains(r#""type":"officeLocation""#), "got: {json}");
    assert!(
        json.contains(r#""asyncOperation":"inProgress""#),
        "got: {json}"
    );
    // NOTE: the empty half of the extended properties stays off the
    // wire, so a write does not clear the shared one.
    assert!(!json.contains(r#""shared""#), "got: {json}");

    let reparsed: GcalEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, reparsed);
}

#[test]
fn parses_a_home_office_working_location() {
    let json = r#"{"workingLocationProperties":{"type":"homeOffice","homeOffice":{}}}"#;
    let event: GcalEvent = serde_json::from_str(json).unwrap();
    let properties = event.working_location_properties.unwrap();

    assert_eq!(
        properties.working_location_type,
        Some(GcalEventWorkingLocationType::HomeOffice)
    );
    assert!(properties.home_office.is_some());
}

#[test]
fn parses_a_pending_conference_create_request() {
    let json = r#"{"conferenceData":{"createRequest":{"requestId":"req-1","conferenceSolutionKey":{"type":"hangoutsMeet"},"status":{"statusCode":"pending"}}}}"#;
    let event: GcalEvent = serde_json::from_str(json).unwrap();
    let request = event.conference_data.unwrap().create_request.unwrap();

    assert_eq!(request.request_id.as_deref(), Some("req-1"));
    assert_eq!(
        request.status.and_then(|status| status.status_code),
        Some(GcalConferenceRequestStatusCode::Pending)
    );
}

#[test]
fn parses_an_empty_events_collection() {
    let events: GcalEvents = serde_json::from_str("{}").unwrap();

    assert!(events.items.is_empty());
    assert!(events.default_reminders.is_empty());
    assert!(events.next_sync_token.is_none());
}

#[test]
fn spells_out_every_event_enum() {
    fn round_trip<T>(wire: &str, value: T)
    where
        T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + core::fmt::Debug,
    {
        let json = format!(r#""{wire}""#);
        assert_eq!(serde_json::to_string(&value).unwrap(), json, "{wire}");
        assert_eq!(serde_json::from_str::<T>(&json).unwrap(), value, "{wire}");
    }

    round_trip("confirmed", GcalEventStatus::Confirmed);
    round_trip("tentative", GcalEventStatus::Tentative);
    round_trip("cancelled", GcalEventStatus::Cancelled);

    round_trip("opaque", GcalEventTransparency::Opaque);
    round_trip("transparent", GcalEventTransparency::Transparent);

    round_trip("default", GcalEventVisibility::Default);
    round_trip("public", GcalEventVisibility::Public);
    round_trip("private", GcalEventVisibility::Private);
    round_trip("confidential", GcalEventVisibility::Confidential);

    round_trip("birthday", GcalEventType::Birthday);
    round_trip("default", GcalEventType::Default);
    round_trip("focusTime", GcalEventType::FocusTime);
    round_trip("fromGmail", GcalEventType::FromGmail);
    round_trip("outOfOffice", GcalEventType::OutOfOffice);
    round_trip("workingLocation", GcalEventType::WorkingLocation);

    round_trip("needsAction", GcalEventAttendeeResponseStatus::NeedsAction);
    round_trip("declined", GcalEventAttendeeResponseStatus::Declined);
    round_trip("tentative", GcalEventAttendeeResponseStatus::Tentative);
    round_trip("accepted", GcalEventAttendeeResponseStatus::Accepted);

    round_trip("email", GcalEventReminderMethod::Email);
    round_trip("popup", GcalEventReminderMethod::Popup);

    round_trip("anniversary", GcalEventBirthdayType::Anniversary);
    round_trip("birthday", GcalEventBirthdayType::Birthday);
    round_trip("custom", GcalEventBirthdayType::Custom);
    round_trip("other", GcalEventBirthdayType::Other);
    // NOTE: `self` is a Rust keyword, so the owner's own birthday is
    // the one variant whose name cannot mirror the wire value.
    round_trip("self", GcalEventBirthdayType::Own);

    round_trip("declineNone", GcalEventAutoDeclineMode::DeclineNone);
    round_trip(
        "declineAllConflictingInvitations",
        GcalEventAutoDeclineMode::DeclineAllConflictingInvitations,
    );
    round_trip(
        "declineOnlyNewConflictingInvitations",
        GcalEventAutoDeclineMode::DeclineOnlyNewConflictingInvitations,
    );

    round_trip("available", GcalEventChatStatus::Available);
    round_trip("doNotDisturb", GcalEventChatStatus::DoNotDisturb);

    round_trip("homeOffice", GcalEventWorkingLocationType::HomeOffice);
    round_trip(
        "officeLocation",
        GcalEventWorkingLocationType::OfficeLocation,
    );
    round_trip(
        "customLocation",
        GcalEventWorkingLocationType::CustomLocation,
    );

    round_trip("video", GcalEntryPointType::Video);
    round_trip("phone", GcalEntryPointType::Phone);
    round_trip("sip", GcalEntryPointType::Sip);
    round_trip("more", GcalEntryPointType::More);

    round_trip("pending", GcalConferenceRequestStatusCode::Pending);
    round_trip("success", GcalConferenceRequestStatusCode::Success);
    round_trip("failure", GcalConferenceRequestStatusCode::Failure);
}

#[test]
fn spells_out_the_send_updates_values() {
    let mut written = Vec::new();

    for (wire, value) in [
        ("all", GcalSendUpdates::All),
        ("externalOnly", GcalSendUpdates::ExternalOnly),
        ("none", GcalSendUpdates::None),
    ] {
        let mut coroutine =
            GcalEventDelete::new(&auth(), "primary", "ev1", Some(value), None).unwrap();
        let (request, ret) = expect_exchange(&mut coroutine, &empty_response("204 No Content"));

        ret.unwrap();

        assert_query(&request, &format!("sendUpdates={wire}"));
        written.push(wire);
    }

    assert_eq!(written.len(), 3);
}
