//! Offline coverage of the request path: every coroutine taking an id
//! writes it as one encoded segment, and refuses an id that names no
//! segment.

mod common;

use core::fmt::Debug;

use common::*;
use io_gcal::{
    coroutine::*,
    v3::{
        rest::{
            acl::{
                GcalAclRule, delete::GcalAclRuleDelete, get::GcalAclRuleGet,
                insert::GcalAclRuleInsert, list::GcalAclList, patch::GcalAclRulePatch,
                update::GcalAclRuleUpdate, watch::GcalAclWatch,
            },
            calendar_list::{
                GcalCalendarListEntry, delete::GcalCalendarListEntryDelete,
                get::GcalCalendarListEntryGet, patch::GcalCalendarListEntryPatch,
                update::GcalCalendarListEntryUpdate,
            },
            calendars::{
                GcalCalendar, clear::GcalCalendarClear, delete::GcalCalendarDelete,
                get::GcalCalendarGet, patch::GcalCalendarPatch,
                transfer_ownership::GcalCalendarTransferOwnership, update::GcalCalendarUpdate,
            },
            channels::GcalChannel,
            events::{
                GcalEvent, GcalEventDateTime, delete::GcalEventDelete, get::GcalEventGet,
                import::GcalEventImport, insert::GcalEventInsert, instances::GcalEventInstances,
                list::GcalEventsList, r#move::GcalEventMove, patch::GcalEventPatch,
                quick_add::GcalEventQuickAdd, update::GcalEventUpdate, watch::GcalEventsWatch,
            },
            settings::get::GcalSettingGet,
        },
        send::GcalSendError,
    },
};

/// The path of the request a coroutine writes, after its method.
fn path<C, R>(mut coroutine: C) -> String
where
    C: GcalCoroutine<Yield = GcalYield, Return = R>,
    R: Debug,
{
    let bytes = expect_wants_write(&mut coroutine, None);
    let request = String::from_utf8_lossy(&bytes).into_owned();
    let line = request_line(&request);

    line.split('?').next().unwrap_or_default().to_string()
}

/// Every request whose path takes an id, as the template of its path
/// (`{c}` the calendar, `{i}` the other id) and the path it writes.
fn paths(calendar: &str, id: &str) -> Vec<(&'static str, Result<String, GcalSendError>)> {
    let auth = auth();
    let boundary = |stamp: &str| {
        Some(GcalEventDateTime {
            date_time: Some(String::from(stamp)),
            ..Default::default()
        })
    };
    let event = GcalEvent {
        ical_uid: Some(String::from("ev1@example.org")),
        start: boundary("2026-08-11T10:00:00Z"),
        end: boundary("2026-08-11T11:00:00Z"),
        ..Default::default()
    };
    let rule = GcalAclRule::default();
    let entry = GcalCalendarListEntry::default();
    let channel = GcalChannel::default();
    let resource = GcalCalendar::default();

    vec![
        (
            "GET /calendar/v3/calendars/{c}/acl",
            GcalAclList::new(&auth, calendar, &Default::default()).map(path),
        ),
        (
            "POST /calendar/v3/calendars/{c}/acl",
            GcalAclRuleInsert::new(&auth, calendar, &rule, None).map(path),
        ),
        (
            "POST /calendar/v3/calendars/{c}/acl/watch",
            GcalAclWatch::new(&auth, calendar, &channel, &Default::default()).map(path),
        ),
        (
            "GET /calendar/v3/calendars/{c}/acl/{i}",
            GcalAclRuleGet::new(&auth, calendar, id).map(path),
        ),
        (
            "PUT /calendar/v3/calendars/{c}/acl/{i}",
            GcalAclRuleUpdate::new(&auth, calendar, id, &rule, None).map(path),
        ),
        (
            "PATCH /calendar/v3/calendars/{c}/acl/{i}",
            GcalAclRulePatch::new(&auth, calendar, id, &rule, None).map(path),
        ),
        (
            "DELETE /calendar/v3/calendars/{c}/acl/{i}",
            GcalAclRuleDelete::new(&auth, calendar, id).map(path),
        ),
        (
            "GET /calendar/v3/users/me/calendarList/{c}",
            GcalCalendarListEntryGet::new(&auth, calendar).map(path),
        ),
        (
            "PUT /calendar/v3/users/me/calendarList/{c}",
            GcalCalendarListEntryUpdate::new(&auth, calendar, &entry, None).map(path),
        ),
        (
            "PATCH /calendar/v3/users/me/calendarList/{c}",
            GcalCalendarListEntryPatch::new(&auth, calendar, &entry, None).map(path),
        ),
        (
            "DELETE /calendar/v3/users/me/calendarList/{c}",
            GcalCalendarListEntryDelete::new(&auth, calendar).map(path),
        ),
        (
            "GET /calendar/v3/calendars/{c}",
            GcalCalendarGet::new(&auth, calendar).map(path),
        ),
        (
            "PUT /calendar/v3/calendars/{c}",
            GcalCalendarUpdate::new(&auth, calendar, &resource).map(path),
        ),
        (
            "PATCH /calendar/v3/calendars/{c}",
            GcalCalendarPatch::new(&auth, calendar, &resource).map(path),
        ),
        (
            "DELETE /calendar/v3/calendars/{c}",
            GcalCalendarDelete::new(&auth, calendar).map(path),
        ),
        (
            "POST /calendar/v3/calendars/{c}/clear",
            GcalCalendarClear::new(&auth, calendar).map(path),
        ),
        (
            "POST /calendar/v3/calendars/{c}/transferOwnership",
            GcalCalendarTransferOwnership::new(&auth, calendar, "new@example.org", false).map(path),
        ),
        (
            "GET /calendar/v3/calendars/{c}/events",
            GcalEventsList::new(&auth, calendar, &Default::default()).map(path),
        ),
        (
            "POST /calendar/v3/calendars/{c}/events",
            GcalEventInsert::new(&auth, calendar, &event, &Default::default()).map(path),
        ),
        (
            "POST /calendar/v3/calendars/{c}/events/import",
            GcalEventImport::new(&auth, calendar, &event, &Default::default()).map(path),
        ),
        (
            "POST /calendar/v3/calendars/{c}/events/quickAdd",
            GcalEventQuickAdd::new(&auth, calendar, "Lunch", None).map(path),
        ),
        (
            "POST /calendar/v3/calendars/{c}/events/watch",
            GcalEventsWatch::new(&auth, calendar, &channel, &Default::default()).map(path),
        ),
        (
            "GET /calendar/v3/calendars/{c}/events/{i}",
            GcalEventGet::new(&auth, calendar, id, None, None).map(path),
        ),
        (
            "PUT /calendar/v3/calendars/{c}/events/{i}",
            GcalEventUpdate::new(&auth, calendar, id, &event, &Default::default(), None).map(path),
        ),
        (
            "PATCH /calendar/v3/calendars/{c}/events/{i}",
            GcalEventPatch::new(&auth, calendar, id, &event, &Default::default(), None).map(path),
        ),
        (
            "DELETE /calendar/v3/calendars/{c}/events/{i}",
            GcalEventDelete::new(&auth, calendar, id, None, None).map(path),
        ),
        (
            "GET /calendar/v3/calendars/{c}/events/{i}/instances",
            GcalEventInstances::new(&auth, calendar, id, &Default::default()).map(path),
        ),
        (
            "POST /calendar/v3/calendars/{c}/events/{i}/move",
            GcalEventMove::new(&auth, calendar, id, "other@example.org", None).map(path),
        ),
        (
            "GET /calendar/v3/users/me/settings/{i}",
            GcalSettingGet::new(&auth, id).map(path),
        ),
    ]
}

#[test]
fn writes_every_id_as_one_encoded_segment() {
    // NOTE: the first is an id the provider mints for a subscribed
    // calendar, the second holds every separator of a URL.
    for (id, segment) in [
        (
            "en.usa#holiday@group.v.calendar.google.com",
            "en.usa%23holiday@group.v.calendar.google.com",
        ),
        ("../a/b?c#d %", "..%2Fa%2Fb%3Fc%23d%20%25"),
    ] {
        let paths = paths(id, id);
        assert_eq!(paths.len(), 29);

        for (template, path) in paths {
            let expected = template.replace("{c}", segment).replace("{i}", segment);
            assert_eq!(path.unwrap(), expected);
        }
    }
}

#[test]
fn refuses_an_id_that_names_no_segment() {
    for id in ["", ".", "..", "a\tb", "a\nb", "a\rb"] {
        for (calendar, other) in [(id, "ok"), ("ok", id)] {
            for (template, path) in paths(calendar, other) {
                let refused = template.contains("{c}") && calendar == id
                    || template.contains("{i}") && other == id;

                match path {
                    Err(GcalSendError::InvalidRequest(_)) if refused => {}
                    Ok(path) if !refused => {
                        assert_eq!(path, template.replace("{c}", "ok").replace("{i}", "ok"));
                    }
                    path => panic!("{id:?} in {template}: {path:?}"),
                }
            }
        }
    }
}
