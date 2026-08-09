//! Offline coverage of the calendars (`calendars`): the seven methods,
//! their validation and the calendar representation.

mod common;

use common::*;
use io_gcal::v3::{
    rest::calendars::{
        GcalCalendar, GcalConferenceProperties, GcalConferenceSolutionType, GcalEventLabel,
        GcalLabelProperties, clear::GcalCalendarClear, delete::GcalCalendarDelete,
        get::GcalCalendarGet, insert::GcalCalendarInsert, patch::GcalCalendarPatch,
        transfer_ownership::GcalCalendarTransferOwnership, update::GcalCalendarUpdate,
    },
    send::GcalSendError,
};

const CALENDAR: &str = r#"{"kind":"calendar#calendar","etag":"\"tag-1\"","id":"cal-1@group.calendar.google.com","summary":"Team","description":"The team calendar","location":"Paris","timeZone":"Europe/Paris","conferenceProperties":{"allowedConferenceSolutionTypes":["hangoutsMeet"]}}"#;

#[test]
fn gets_a_calendar() {
    let mut coroutine = GcalCalendarGet::new(&auth(), "primary").unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", CALENDAR));
    let out = ret.unwrap();

    assert_eq!(out.response.summary.as_deref(), Some("Team"));
    assert_eq!(out.response.time_zone.as_deref(), Some("Europe/Paris"));
    assert_eq!(
        out.response
            .conference_properties
            .map(|properties| properties.allowed_conference_solution_types),
        Some(vec![GcalConferenceSolutionType::HangoutsMeet])
    );

    assert_eq!(request_line(&request), "GET /calendar/v3/calendars/primary");
}

#[test]
fn inserts_a_calendar() {
    let calendar = GcalCalendar {
        summary: Some(String::from("Team")),
        time_zone: Some(String::from("Europe/Paris")),
        ..Default::default()
    };
    let mut coroutine = GcalCalendarInsert::new(&auth(), &calendar).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", CALENDAR));
    let out = ret.unwrap();

    assert_eq!(
        out.response.id.as_deref(),
        Some("cal-1@group.calendar.google.com")
    );

    assert_eq!(request_line(&request), "POST /calendar/v3/calendars");
    assert_eq!(
        request_body(&request),
        r#"{"summary":"Team","timeZone":"Europe/Paris"}"#
    );
}

#[test]
fn refuses_a_calendar_without_a_summary() {
    for calendar in [
        GcalCalendar::default(),
        GcalCalendar {
            summary: Some(String::from("   ")),
            ..Default::default()
        },
    ] {
        let Err(err) = GcalCalendarInsert::new(&auth(), &calendar) else {
            panic!("expected an invalid request error");
        };

        assert!(
            matches!(err, GcalSendError::InvalidRequest(ref message) if message.contains("summary")),
            "got: {err}"
        );
    }
}

#[test]
fn updates_a_calendar() {
    let calendar = GcalCalendar {
        summary: Some(String::from("Renamed")),
        ..Default::default()
    };
    let mut coroutine = GcalCalendarUpdate::new(&auth(), "cal-1", &calendar).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", CALENDAR));

    ret.unwrap();

    assert_eq!(request_line(&request), "PUT /calendar/v3/calendars/cal-1");
    assert_eq!(request_body(&request), r#"{"summary":"Renamed"}"#);
}

#[test]
fn patches_a_calendar() {
    let calendar = GcalCalendar {
        description: Some(String::from("Patched")),
        ..Default::default()
    };
    let mut coroutine = GcalCalendarPatch::new(&auth(), "cal-1", &calendar).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", CALENDAR));

    ret.unwrap();

    assert_eq!(request_line(&request), "PATCH /calendar/v3/calendars/cal-1");
    assert_eq!(request_body(&request), r#"{"description":"Patched"}"#);
}

#[test]
fn deletes_a_calendar() {
    let mut coroutine = GcalCalendarDelete::new(&auth(), "cal-1").unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &empty_response("204 No Content"));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "DELETE /calendar/v3/calendars/cal-1"
    );
}

#[test]
fn clears_a_calendar() {
    let mut coroutine = GcalCalendarClear::new(&auth(), "primary").unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &empty_response("204 No Content"));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "POST /calendar/v3/calendars/primary/clear"
    );
    // NOTE: a bodiless POST still announces its content type, and its
    // zero length, or the server waits for a body that never comes.
    assert!(request.contains("content-length: 0"), "got: {request}");
}

#[test]
fn transfers_the_ownership_of_a_calendar() {
    let mut coroutine =
        GcalCalendarTransferOwnership::new(&auth(), "cal-1", "new@example.org", true).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &empty_response("204 No Content"));

    ret.unwrap();

    assert!(
        request_line(&request).starts_with("POST /calendar/v3/calendars/cal-1/transferOwnership?"),
        "got: {request}"
    );
    assert_query(&request, "newDataOwner=new%40example.org");
    assert_query(&request, "useAdminAccess=true");
}

#[test]
fn round_trips_the_event_labels_of_a_calendar() {
    let calendar = GcalCalendar {
        summary: Some(String::from("Labelled")),
        label_properties: Some(GcalLabelProperties {
            event_labels: vec![GcalEventLabel {
                name: Some(String::from("Focus")),
                background_color: Some(String::from("#039be5")),
                ..Default::default()
            }],
        }),
        conference_properties: Some(GcalConferenceProperties {
            allowed_conference_solution_types: vec![
                GcalConferenceSolutionType::HangoutsMeet,
                GcalConferenceSolutionType::AddOn,
            ],
        }),
        ..Default::default()
    };

    let json = serde_json::to_string(&calendar).unwrap();
    assert!(
        json.contains(r##""backgroundColor":"#039be5""##),
        "got: {json}"
    );
    assert!(
        json.contains(r#""allowedConferenceSolutionTypes":["hangoutsMeet","addOn"]"#),
        "got: {json}"
    );

    let reparsed: GcalCalendar = serde_json::from_str(&json).unwrap();
    assert_eq!(calendar, reparsed);
}

#[test]
fn spells_out_every_conference_solution_type() {
    let types = [
        ("eventHangout", GcalConferenceSolutionType::EventHangout),
        (
            "eventNamedHangout",
            GcalConferenceSolutionType::EventNamedHangout,
        ),
        ("hangoutsMeet", GcalConferenceSolutionType::HangoutsMeet),
        ("addOn", GcalConferenceSolutionType::AddOn),
    ];

    for (wire, solution_type) in types {
        let json = format!(r#""{wire}""#);
        assert_eq!(serde_json::to_string(&solution_type).unwrap(), json);
        assert_eq!(
            serde_json::from_str::<GcalConferenceSolutionType>(&json).unwrap(),
            solution_type,
            "{wire}"
        );
    }
}
