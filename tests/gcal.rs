#![cfg(any(
    feature = "rustls-ring",
    feature = "rustls-aws",
    feature = "native-tls"
))]
//! End-to-end Google Calendar API test.
//!
//! Exercises the whole CRUD surface (calendars, calendar list, events,
//! instances, quick add, ACL, free/busy, colours, settings) on a
//! throwaway secondary calendar, and deletes it at the end, leaving the
//! account untouched.
//!
//! Requires an OAuth2 access token with the
//! https://www.googleapis.com/auth/calendar scope:
//!
//! ```sh
//! GCAL_ACCESS_TOKEN="<token>" \
//! cargo test --test gcal -- --include-ignored
//! ```

use std::{
    env,
    time::{SystemTime, UNIX_EPOCH},
};

use io_gcal::v3::rest::{
    calendar_list::list::GcalCalendarListListParams,
    calendars::GcalCalendar,
    events::{
        GcalEvent, GcalEventDateTime, instances::GcalEventInstancesParams,
        list::GcalEventsListParams,
    },
    freebusy::{GcalFreeBusyRequest, GcalFreeBusyRequestItem},
};
use io_gcal::v3::{
    client::{GcalClientStd, GcalClientStdConnectOptions},
    rest::acl::list::GcalAclListParams,
};
use pimalaya_stream::tls::Tls;

#[test]
#[ignore = "requires GCAL_ACCESS_TOKEN env var and --include-ignored"]
fn gcal() {
    env_logger::try_init().ok();

    let token = env::var("GCAL_ACCESS_TOKEN").expect("GCAL_ACCESS_TOKEN not set");

    let options = GcalClientStdConnectOptions {
        tls: Tls::default(),
    };
    let mut client = GcalClientStd::connect(token, options).expect("connect");

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let calendar_summary = format!("io-gcal-test-{ts}");

    // ── COLORS, SETTINGS, CALENDAR LIST (baseline) ───────────────────────────

    let colors = client.colors_get().expect("colors get").response;
    assert!(!colors.event.is_empty(), "the event palette is never empty");

    client
        .settings_list(&Default::default())
        .expect("settings list");

    client
        .calendar_list_list(&GcalCalendarListListParams::default())
        .expect("calendar list list");

    // ── CALENDAR CREATE ──────────────────────────────────────────────────────

    let calendar = GcalCalendar {
        summary: Some(calendar_summary.clone()),
        time_zone: Some(String::from("Europe/Paris")),
        ..Default::default()
    };
    let created = client
        .calendar_insert(&calendar)
        .expect("calendar insert")
        .response;
    let calendar_id = created.id.clone().expect("created calendar carries an id");

    // Everything below runs against the throwaway calendar, so a failure
    // still leaves the account with a single stray calendar at worst.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        exercise(&mut client, &calendar_id, &calendar_summary);
    }));

    // ── CALENDAR DELETE (cleanup) ────────────────────────────────────────────

    client
        .calendar_delete(&calendar_id)
        .expect("calendar delete");

    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}

fn exercise(client: &mut GcalClientStd, calendar_id: &str, calendar_summary: &str) {
    // ── CALENDAR READ AND PATCH ──────────────────────────────────────────────

    let fetched = client
        .calendar_get(calendar_id)
        .expect("calendar get")
        .response;
    assert_eq!(fetched.summary.as_deref(), Some(calendar_summary));

    let patch = GcalCalendar {
        description: Some(String::from("patched by the io-gcal test suite")),
        ..Default::default()
    };
    let patched = client
        .calendar_patch(calendar_id, &patch)
        .expect("calendar patch")
        .response;
    assert_eq!(
        patched.description.as_deref(),
        Some("patched by the io-gcal test suite")
    );

    // ── CALENDAR LIST ENTRY ──────────────────────────────────────────────────

    let entry = client
        .calendar_list_entry_get(calendar_id)
        .expect("calendar list entry get")
        .response;
    assert_eq!(entry.id.as_deref(), Some(calendar_id));

    // ── EVENT CREATE, READ, PATCH, UPDATE ────────────────────────────────────

    let event = GcalEvent {
        summary: Some(String::from("io-gcal test event")),
        start: Some(GcalEventDateTime {
            date_time: Some(String::from("2030-01-06T10:00:00+01:00")),
            ..Default::default()
        }),
        end: Some(GcalEventDateTime {
            date_time: Some(String::from("2030-01-06T11:00:00+01:00")),
            ..Default::default()
        }),
        ..Default::default()
    };
    let created = client
        .event_insert(calendar_id, &event, &Default::default())
        .expect("event insert")
        .response;
    let event_id = created.id.clone().expect("created event carries an id");

    let fetched = client
        .event_get(calendar_id, &event_id, None, None)
        .expect("event get")
        .response;
    assert_eq!(fetched.summary.as_deref(), Some("io-gcal test event"));

    let patch = GcalEvent {
        location: Some(String::from("Somewhere")),
        ..Default::default()
    };
    let patched = client
        .event_patch(calendar_id, &event_id, &patch, &Default::default())
        .expect("event patch")
        .response;
    assert_eq!(patched.location.as_deref(), Some("Somewhere"));

    let mut replacement = patched.clone();
    replacement.summary = Some(String::from("io-gcal test event renamed"));
    let updated = client
        .event_update(calendar_id, &event_id, &replacement, &Default::default())
        .expect("event update")
        .response;
    assert_eq!(
        updated.summary.as_deref(),
        Some("io-gcal test event renamed")
    );

    // ── RECURRING EVENT AND ITS INSTANCES ────────────────────────────────────

    let recurring = GcalEvent {
        summary: Some(String::from("io-gcal test series")),
        start: Some(GcalEventDateTime {
            date_time: Some(String::from("2030-02-04T10:00:00+01:00")),
            ..Default::default()
        }),
        end: Some(GcalEventDateTime {
            date_time: Some(String::from("2030-02-04T10:30:00+01:00")),
            ..Default::default()
        }),
        recurrence: vec![String::from("RRULE:FREQ=WEEKLY;COUNT=3")],
        ..Default::default()
    };
    let series = client
        .event_insert(calendar_id, &recurring, &Default::default())
        .expect("recurring event insert")
        .response;
    let series_id = series.id.clone().expect("created series carries an id");

    let instances = client
        .event_instances(
            calendar_id,
            &series_id,
            &GcalEventInstancesParams::default(),
        )
        .expect("event instances")
        .response;
    assert_eq!(instances.items.len(), 3);

    // ── QUICK ADD ────────────────────────────────────────────────────────────

    let quick = client
        .event_quick_add(calendar_id, "io-gcal quick event on 2030-03-05 10am", None)
        .expect("event quick add")
        .response;
    assert!(quick.id.is_some());

    // ── LISTING AND INCREMENTAL SYNC ─────────────────────────────────────────

    let params = GcalEventsListParams {
        single_events: true,
        max_results: Some(50),
        ..Default::default()
    };
    let listed = client
        .events_list(calendar_id, &params)
        .expect("events list")
        .response;
    assert!(listed.items.len() >= 4, "got {} events", listed.items.len());

    let sync_token = listed
        .next_sync_token
        .clone()
        .expect("the last page carries a sync token");

    client
        .event_delete(calendar_id, &event_id, None)
        .expect("event delete");

    let params = GcalEventsListParams {
        single_events: true,
        sync_token: Some(&sync_token),
        ..Default::default()
    };
    let changed = client
        .events_list(calendar_id, &params)
        .expect("incremental events list")
        .response;
    assert!(
        changed
            .items
            .iter()
            .any(|event| event.id.as_deref() == Some(&event_id)),
        "the incremental listing reports the deleted event"
    );

    // ── ACL AND FREE/BUSY ────────────────────────────────────────────────────

    let acl = client
        .acl_list(calendar_id, &GcalAclListParams::default())
        .expect("acl list")
        .response;
    assert!(!acl.items.is_empty(), "the owner rule always exists");

    let request = GcalFreeBusyRequest {
        time_min: Some(String::from("2030-02-01T00:00:00Z")),
        time_max: Some(String::from("2030-03-01T00:00:00Z")),
        items: vec![GcalFreeBusyRequestItem {
            id: Some(String::from(calendar_id)),
        }],
        ..Default::default()
    };
    let free_busy = client
        .free_busy_query(&request)
        .expect("free/busy query")
        .response;
    assert!(free_busy.calendars.contains_key(calendar_id));
}
