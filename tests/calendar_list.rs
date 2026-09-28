//! Offline coverage of the calendar list (`calendarList`): the seven
//! methods, the colour format switch and the entry representation.

mod common;

use common::*;
use io_gcal::v3::{
    rest::{
        acl::GcalAccessRole,
        calendar_list::{
            GcalCalendarListEntry, GcalCalendarNotification, GcalCalendarNotificationSettings,
            GcalCalendarNotificationType,
            delete::GcalCalendarListEntryDelete,
            get::GcalCalendarListEntryGet,
            insert::GcalCalendarListEntryInsert,
            list::{GcalCalendarListList, GcalCalendarListListParams},
            patch::GcalCalendarListEntryPatch,
            update::GcalCalendarListEntryUpdate,
            watch::GcalCalendarListWatch,
        },
        channels::{GcalChannel, GcalChannelType},
        events::GcalEventReminderMethod,
    },
    send::GcalSendError,
};

const ENTRY: &str = r##"{"kind":"calendar#calendarListEntry","id":"primary","summary":"Jane","accessRole":"owner","primary":true,"selected":true,"backgroundColor":"#0088aa","foregroundColor":"#ffffff","defaultReminders":[{"method":"popup","minutes":10}],"notificationSettings":{"notifications":[{"type":"eventCreation","method":"email"}]}}"##;

#[test]
fn lists_the_calendar_list() {
    let body =
        format!(r#"{{"kind":"calendar#calendarList","items":[{ENTRY}],"nextSyncToken":"s1"}}"#);
    let params = GcalCalendarListListParams {
        max_results: Some(20),
        min_access_role: Some(GcalAccessRole::Writer),
        show_deleted: true,
        show_hidden: true,
        show_own_organization_only: true,
        page_token: Some("p1"),
        sync_token: Some("s0"),
    };
    let mut coroutine = GcalCalendarListList::new(&auth(), &params).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", &body));
    let out = ret.unwrap();

    assert_eq!(out.response.items.len(), 1);
    assert_eq!(
        out.response.items[0].access_role,
        Some(GcalAccessRole::Owner)
    );
    assert_eq!(
        out.response.items[0]
            .default_reminders
            .first()
            .and_then(|reminder| reminder.minutes),
        Some(10)
    );
    assert_eq!(
        out.response.items[0]
            .notification_settings
            .as_ref()
            .and_then(|settings| settings.notifications.first())
            .and_then(|notification| notification.notification_type),
        Some(GcalCalendarNotificationType::EventCreation)
    );
    assert_eq!(out.response.next_sync_token.as_deref(), Some("s1"));

    assert!(
        request_line(&request).starts_with("GET /calendar/v3/users/me/calendarList?"),
        "got: {request}"
    );
    assert_query(&request, "maxResults=20");
    assert_query(&request, "minAccessRole=writer");
    assert_query(&request, "showDeleted=true");
    assert_query(&request, "showHidden=true");
    assert_query(&request, "showOwnOrganizationOnly=true");
    assert_query(&request, "pageToken=p1");
    assert_query(&request, "syncToken=s0");
}

#[test]
fn lists_the_calendar_list_without_any_parameter() {
    let mut coroutine = GcalCalendarListList::new(&auth(), &Default::default()).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", "{}"));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "GET /calendar/v3/users/me/calendarList"
    );
}

#[test]
fn reports_the_entries_an_incremental_listing_deleted() {
    let body = r#"{"items":[{"id":"gone@example.org","deleted":true}]}"#;
    let params = GcalCalendarListListParams {
        sync_token: Some("s0"),
        ..Default::default()
    };
    let mut coroutine = GcalCalendarListList::new(&auth(), &params).unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", body));
    let out = ret.unwrap();

    assert_eq!(out.response.items[0].deleted, Some(true));
}

#[test]
fn gets_an_entry() {
    let mut coroutine = GcalCalendarListEntryGet::new(&auth(), "primary").unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", ENTRY));
    let out = ret.unwrap();

    assert_eq!(out.response.primary, Some(true));
    assert_eq!(out.response.background_color.as_deref(), Some("#0088aa"));

    assert_eq!(
        request_line(&request),
        "GET /calendar/v3/users/me/calendarList/primary"
    );
}

#[test]
fn inserts_an_entry() {
    let entry = GcalCalendarListEntry {
        id: Some(String::from("cal-1@group.calendar.google.com")),
        ..Default::default()
    };
    let mut coroutine = GcalCalendarListEntryInsert::new(&auth(), &entry, None).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", ENTRY));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "POST /calendar/v3/users/me/calendarList"
    );
    assert_eq!(
        request_body(&request),
        r#"{"id":"cal-1@group.calendar.google.com"}"#
    );
}

#[test]
fn inserts_an_entry_with_explicit_colours() {
    let entry = GcalCalendarListEntry {
        id: Some(String::from("cal-1")),
        background_color: Some(String::from("#0088aa")),
        ..Default::default()
    };
    let mut coroutine = GcalCalendarListEntryInsert::new(&auth(), &entry, Some(true)).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", ENTRY));

    ret.unwrap();

    assert_query(&request, "colorRgbFormat=true");
}

#[test]
fn refuses_an_entry_without_a_calendar_id() {
    for entry in [
        GcalCalendarListEntry::default(),
        GcalCalendarListEntry {
            id: Some(String::from("  ")),
            ..Default::default()
        },
    ] {
        let Err(err) = GcalCalendarListEntryInsert::new(&auth(), &entry, None) else {
            panic!("expected an invalid request error");
        };

        assert!(
            matches!(err, GcalSendError::InvalidRequest(ref message) if message.contains("id")),
            "got: {err}"
        );
    }
}

#[test]
fn updates_an_entry() {
    let entry = GcalCalendarListEntry {
        summary_override: Some(String::from("My name for it")),
        ..Default::default()
    };
    let mut coroutine =
        GcalCalendarListEntryUpdate::new(&auth(), "cal-1", &entry, Some(false)).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", ENTRY));

    ret.unwrap();

    assert!(
        request_line(&request).starts_with("PUT /calendar/v3/users/me/calendarList/cal-1?"),
        "got: {request}"
    );
    assert_query(&request, "colorRgbFormat=false");
    assert_eq!(
        request_body(&request),
        r#"{"summaryOverride":"My name for it"}"#
    );
}

#[test]
fn patches_an_entry() {
    let entry = GcalCalendarListEntry {
        hidden: Some(true),
        ..Default::default()
    };
    let mut coroutine = GcalCalendarListEntryPatch::new(&auth(), "cal-1", &entry, None).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", ENTRY));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "PATCH /calendar/v3/users/me/calendarList/cal-1"
    );
    assert_eq!(request_body(&request), r#"{"hidden":true}"#);
}

#[test]
fn patches_an_entry_with_explicit_colours() {
    let entry = GcalCalendarListEntry {
        foreground_color: Some(String::from("#ffffff")),
        ..Default::default()
    };
    let mut coroutine =
        GcalCalendarListEntryPatch::new(&auth(), "cal-1", &entry, Some(true)).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", ENTRY));

    ret.unwrap();

    assert_query(&request, "colorRgbFormat=true");
}

#[test]
fn deletes_an_entry() {
    let mut coroutine = GcalCalendarListEntryDelete::new(&auth(), "cal-1").unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &empty_response("204 No Content"));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "DELETE /calendar/v3/users/me/calendarList/cal-1"
    );
}

#[test]
fn watches_the_calendar_list() {
    let channel = GcalChannel {
        id: Some(String::from("chan-1")),
        channel_type: Some(GcalChannelType::WebHook),
        address: Some(String::from("https://hook.example.org/gcal")),
        ..Default::default()
    };
    let params = GcalCalendarListListParams {
        show_hidden: true,
        ..Default::default()
    };
    let response = json_response("200 OK", r#"{"id":"chan-1","resourceId":"res-1"}"#);
    let mut coroutine = GcalCalendarListWatch::new(&auth(), &channel, &params).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &response);

    ret.unwrap();

    assert!(
        request_line(&request).starts_with("POST /calendar/v3/users/me/calendarList/watch?"),
        "got: {request}"
    );
    assert_query(&request, "showHidden=true");
}

#[test]
fn round_trips_the_notification_settings_of_an_entry() {
    let entry = GcalCalendarListEntry {
        id: Some(String::from("cal-1")),
        notification_settings: Some(GcalCalendarNotificationSettings {
            notifications: vec![GcalCalendarNotification {
                notification_type: Some(GcalCalendarNotificationType::Agenda),
                method: Some(GcalEventReminderMethod::Email),
            }],
        }),
        ..Default::default()
    };

    let json = serde_json::to_string(&entry).unwrap();
    assert!(
        json.contains(r#""notifications":[{"type":"agenda","method":"email"}]"#),
        "got: {json}"
    );

    let reparsed: GcalCalendarListEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(entry, reparsed);
}

#[test]
fn spells_out_every_notification_type() {
    let types = [
        ("eventCreation", GcalCalendarNotificationType::EventCreation),
        ("eventChange", GcalCalendarNotificationType::EventChange),
        (
            "eventCancellation",
            GcalCalendarNotificationType::EventCancellation,
        ),
        ("eventResponse", GcalCalendarNotificationType::EventResponse),
        ("agenda", GcalCalendarNotificationType::Agenda),
    ];

    for (wire, notification_type) in types {
        let json = format!(r#""{wire}""#);
        assert_eq!(serde_json::to_string(&notification_type).unwrap(), json);
        assert_eq!(
            serde_json::from_str::<GcalCalendarNotificationType>(&json).unwrap(),
            notification_type,
            "{wire}"
        );
    }
}
