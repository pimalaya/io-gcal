//! Live tests against the Google Calendar API.
//!
//! Google only accepts an OAuth2 Bearer token (no app passwords), and
//! there are two ways to hand one to these tests.
//!
//! A token from the authorization-code flow, minted by hand, acts as
//! you and dies within the hour:
//!
//! ```sh
//! GCAL_ACCESS_TOKEN="ya29...." \
//! cargo test --features ical --test google -- --ignored
//! ```
//!
//! A service account instead signs its own assertion and trades it for
//! a token whenever one is needed, which is what makes an unattended
//! run possible. Point the tests at its JSON key and they mint the
//! token themselves through io-oauth, so no token is ever handled by a
//! shell, written to a file or passed between CI steps:
//!
//! ```sh
//! GCAL_SERVICE_ACCOUNT_KEY_FILE=key.json \
//! cargo test --features ical --test google -- --ignored
//! ```
//!
//! CI passes the key itself rather than a path, as
//! `GCAL_SERVICE_ACCOUNT_KEY`, since it comes straight out of a secret.
//!
//! Each flow runs twice: as the service account itself, on the
//! calendars it owns, and as a Workspace user the service account acts
//! for through domain-wide delegation, `GCAL_SERVICE_ACCOUNT_SUBJECT`
//! (`google@pimalaya.org` by default, the Pimalaya test user). A token
//! minted by hand serves both passes. The assertions below only state
//! what holds for any principal, and no calendar the run did not create
//! is ever read or written, whichever token is used.
//!
//! [`account`] only reads, and is satisfied by the
//! `calendar.readonly` scope. [`calendar`] walks the whole CRUD
//! surface on throwaway secondary calendars and needs the full
//! `calendar` scope; everything it creates lives inside those
//! calendars, and [`with_cleanup`] deletes them however the run ends,
//! so a failure leaves the account with stray calendars at worst.
//! [`subscription`] shares such a calendar from the service account
//! with the delegated user, and only runs on a service account key.
//! With the `ical` feature, [`ical`] round-trips an event through the
//! iCalendar projection.
//!
//! The push channels are deliberately left out: a `watch` needs a
//! publicly reachable HTTPS webhook Google can POST to, which a test
//! process cannot provide. So are `calendars.transferOwnership`, which
//! hands a calendar to another principal, and `calendars.clear`, which
//! only applies to a primary calendar and refuses the service account's
//! own (403), leaving a human's whole agenda as the only target.

#![cfg(any(
    feature = "rustls-ring",
    feature = "rustls-aws",
    feature = "native-tls"
))]

use core::fmt::Debug;

use std::{
    borrow::Cow,
    env, fs,
    panic::{self, AssertUnwindSafe},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use io_gcal::v3::{
    client::{GcalClientStd, GcalClientStdConnectOptions, GcalClientStdError},
    rest::{
        acl::{GcalAccessRole, GcalAclRule, GcalAclScope, GcalAclScopeType},
        calendar_list::GcalCalendarListEntry,
        calendars::GcalCalendar,
        events::{
            GcalEvent, GcalEventDateTime, GcalEventStatus, instances::GcalEventInstancesParams,
            list::GcalEventsListParams,
        },
        freebusy::{GcalFreeBusyRequest, GcalFreeBusyRequestItem},
    },
};
use io_oauth::{
    client::Oauth20ClientStd,
    rfc7523::{
        assertion::{Oauth20JwtBearerClaims, Oauth20JwtBearerKey},
        auth_grant::Oauth20JwtBearerGrantRequestParams,
    },
};
use pimalaya_stream::tls::Tls;
use secrecy::ExposeSecret;
use serde::Deserialize;
use url::Url;

/// The scope the live tests need: read and write on the calendars the
/// principal owns.
const CALENDAR_SCOPE: &str = "https://www.googleapis.com/auth/calendar";

/// The Pimalaya Workspace user the service account acts as when
/// `GCAL_SERVICE_ACCOUNT_SUBJECT` names none.
const DEFAULT_SUBJECT: &str = "google@pimalaya.org";

/// Read-only pass as the service account.
#[test]
#[ignore = "requires GCAL_ACCESS_TOKEN env var and --ignored"]
fn account() {
    account_flow(&mut connect(Principal::ServiceAccount));
}

/// Read-only pass as the delegated user.
#[test]
#[ignore = "requires GCAL_ACCESS_TOKEN env var and --ignored"]
fn delegated_account() {
    account_flow(&mut connect(Principal::Delegated));
}

/// Full CRUD pass as the service account.
#[test]
#[ignore = "requires GCAL_ACCESS_TOKEN env var and --ignored"]
fn calendar() {
    calendar_flow(&mut connect(Principal::ServiceAccount));
}

/// Full CRUD pass as the delegated user.
#[test]
#[ignore = "requires GCAL_ACCESS_TOKEN env var and --ignored"]
fn delegated_calendar() {
    calendar_flow(&mut connect(Principal::Delegated));
}

/// Subscribes the delegated user to a calendar of the service account.
///
/// A data owner cannot remove its own calendars from its calendar list,
/// so inserting and deleting an entry takes a calendar someone else
/// owns: the service account shares a throwaway one with the delegated
/// user, who adds it to their list, rewrites the entry, then removes
/// it. A token minted by hand is a single principal, so the test only
/// runs on a service account key.
#[test]
#[ignore = "requires a service account key and --ignored"]
fn subscription() {
    if service_account_key().is_none() {
        eprintln!("skipped: needs two principals, hence a service account key");
        return;
    }

    let mut owner = connect(Principal::ServiceAccount);
    let mut subscriber = connect(Principal::Delegated);
    let id = calendar_create(&mut owner, &format!("io-gcal-test-{}", unix_millis()));

    with_cleanup(
        &mut owner,
        |owner| {
            let rule = GcalAclRule {
                role: Some(GcalAccessRole::Reader),
                scope: Some(GcalAclScope {
                    scope_type: Some(GcalAclScopeType::User),
                    value: Some(subject()),
                }),
                ..Default::default()
            };
            retry("acl rule insert", || {
                owner.acl_rule_insert(&id, &rule, Some(false))
            });

            calendar_list(&mut subscriber, &id);
        },
        |owner| {
            if let Err(err) = owner.calendar_delete(&id) {
                report_leftover("calendar", &id, &err);
            }
        },
    );
}

/// Imports an event from an iCalendar document into a throwaway
/// calendar and reads it back as one, then edits the document and
/// writes the change merged onto the server copy.
///
/// Whatever Google normalizes or drops shows as a difference between
/// the projection of the document written and that of the document
/// read back. Needs the `ical` feature.
#[cfg(feature = "ical")]
#[test]
#[ignore = "requires GCAL_ACCESS_TOKEN env var and --ignored"]
fn ical() {
    let mut client = connect(Principal::ServiceAccount);
    let summary = format!("io-gcal-test-{}", unix_millis());
    let uid = format!("{summary}@pimalaya.org");
    let id = calendar_create(&mut client, &summary);

    with_cleanup(
        &mut client,
        |client| {
            let document = ical_document(&uid, "io-gcal ical", true);
            let written = GcalEvent::from_ical(document.as_bytes()).expect("the document projects");
            let imported = retry("event import", || {
                client.event_import(&id, &written, &Default::default())
            })
            .response;
            let event_id = imported
                .id
                .clone()
                .expect("the imported event carries an id");

            let fetched =
                retry("event get", || client.event_get(&id, &event_id, None, None)).response;
            let read = fetched.to_ical();
            assert!(
                read.contains("X-PIMALAYA-TEST:kept verbatim"),
                "the stash restores the unmanaged lines:\n{read}"
            );
            let back = GcalEvent::from_ical(read.as_bytes()).expect("the read document projects");
            assert_same_projection(&written, &back, &document, &read);

            let edited_document = ical_document(&uid, "io-gcal ical renamed", false);
            let edited = GcalEvent::from_ical(edited_document.as_bytes())
                .expect("the edited document projects");
            let updated = retry("event update", || {
                client.event_update(
                    &id,
                    &event_id,
                    &edited.clone().merge(&fetched),
                    &Default::default(),
                    fetched.etag.as_deref(),
                )
            })
            .response;
            let reread = updated.to_ical();
            let back =
                GcalEvent::from_ical(reread.as_bytes()).expect("the updated document projects");
            assert_same_projection(&edited, &back, &edited_document, &reread);
        },
        |client| {
            if let Err(err) = client.calendar_delete(&id) {
                report_leftover("calendar", &id, &err);
            }
        },
    );
}

/// Reads the account-wide resources: the colour palettes, the user
/// settings and the calendar list.
fn account_flow(client: &mut GcalClientStd) {
    let colors = retry("colors get", || client.colors_get()).response;
    assert!(!colors.event.is_empty(), "the event palette is never empty");
    assert!(
        !colors.calendar.is_empty(),
        "the calendar palette is never empty"
    );

    // NOTE: a human account always has settings and a primary
    // calendar; a service account starts with neither, so what is
    // asserted here is the shape of whatever comes back rather than its
    // presence. That a listing can be walked and an entry fetched by id
    // is covered against a calendar the run owns, in [`calendar`].
    let settings = retry("settings list", || {
        client.settings_list(&Default::default())
    })
    .response;
    assert!(
        settings
            .items
            .iter()
            .all(|setting| setting.id.is_some() && setting.value.is_some()),
        "every setting carries an id and a value"
    );

    if let Some(listed) = settings.items.first() {
        let id = listed.id.as_deref().expect("the setting carries an id");
        let fetched = client.setting_get(id).expect("setting get").response;
        assert_eq!(fetched.value, listed.value, "setting get value mismatch");
    }

    let calendars = retry("calendar list list", || {
        client.calendar_list_list(&Default::default())
    })
    .response;
    assert!(
        calendars
            .items
            .iter()
            .all(|entry| entry.id.is_some() && entry.access_role.is_some()),
        "every calendar list entry carries an id and an access role"
    );
}

/// Walks the CRUD surface on two throwaway secondary calendars: the
/// main one, and the destination of an event move.
fn calendar_flow(client: &mut GcalClientStd) {
    let summary = format!("io-gcal-test-{}", unix_millis());
    let id = calendar_create(client, &summary);

    with_cleanup(
        client,
        |client| {
            calendar_metadata(client, &id, &summary);
            events(client, &id, &summary);

            let other = calendar_create(client, &format!("{summary}-other"));

            with_cleanup(
                client,
                |client| {
                    event_move(client, &id, &other);
                },
                |client| {
                    if let Err(err) = client.calendar_delete(&other) {
                        report_leftover("calendar", &other, &err);
                    }
                },
            );

            sharing(client, &id);
            free_busy(client, &id);
        },
        |client| {
            if let Err(err) = client.calendar_delete(&id) {
                report_leftover("calendar", &id, &err);
            }
        },
    );
}

/// Creates a secondary calendar and returns its id.
fn calendar_create(client: &mut GcalClientStd, summary: &str) -> String {
    let created = retry("calendar insert", || {
        client.calendar_insert(&GcalCalendar {
            summary: Some(summary.to_owned()),
            time_zone: Some(String::from("Europe/Paris")),
            ..Default::default()
        })
    })
    .response;

    created.id.expect("the new calendar carries an id")
}

/// Reads back the calendar, patches then replaces it, and checks the
/// entry the creation added to the calendar list.
fn calendar_metadata(client: &mut GcalClientStd, id: &str, summary: &str) {
    let fetched = retry("calendar get", || client.calendar_get(id)).response;
    assert_eq!(fetched.summary.as_deref(), Some(summary));
    assert_eq!(fetched.time_zone.as_deref(), Some("Europe/Paris"));

    let patched = retry("calendar patch", || {
        client.calendar_patch(
            id,
            &GcalCalendar {
                description: Some(String::from("patched by the io-gcal suite")),
                ..Default::default()
            },
        )
    })
    .response;
    assert_eq!(
        patched.description.as_deref(),
        Some("patched by the io-gcal suite")
    );
    assert_eq!(
        patched.summary.as_deref(),
        Some(summary),
        "a patch leaves the fields it does not carry alone"
    );

    let updated = retry("calendar update", || {
        client.calendar_update(
            id,
            &GcalCalendar {
                summary: Some(summary.to_owned()),
                time_zone: Some(String::from("Europe/Paris")),
                ..Default::default()
            },
        )
    })
    .response;
    assert_eq!(updated.summary.as_deref(), Some(summary));
    assert_eq!(
        updated.description, None,
        "an update drops the fields it does not carry"
    );

    let entry = retry("calendar list entry get", || {
        client.calendar_list_entry_get(id)
    })
    .response;
    assert_eq!(entry.access_role, Some(GcalAccessRole::Owner));

    let renamed = retry("calendar list entry patch", || {
        client.calendar_list_entry_patch(
            id,
            &GcalCalendarListEntry {
                summary_override: Some(String::from("io-gcal override")),
                ..Default::default()
            },
            None,
        )
    })
    .response;
    assert_eq!(
        renamed.summary_override.as_deref(),
        Some("io-gcal override")
    );
}

/// Creates, imports, reads, writes, expands and deletes events, then
/// replays the listing incrementally to see the deletion reported.
fn events(client: &mut GcalClientStd, calendar_id: &str, summary: &str) {
    let event = retry("event insert", || {
        client.event_insert(
            calendar_id,
            &GcalEvent {
                summary: Some(String::from("io-gcal test event")),
                start: Some(timed("2030-01-06T10:00:00+01:00")),
                end: Some(timed("2030-01-06T11:00:00+01:00")),
                ..Default::default()
            },
            &Default::default(),
        )
    })
    .response;
    let event_id = event.id.clone().expect("the new event carries an id");

    let fetched = retry("event get", || {
        client.event_get(calendar_id, &event_id, None, None)
    })
    .response;
    assert_eq!(fetched.summary.as_deref(), Some("io-gcal test event"));
    assert!(fetched.ical_uid.is_some(), "the API assigns an iCalUID");

    let patched = retry("event patch", || {
        client.event_patch(
            calendar_id,
            &event_id,
            &GcalEvent {
                location: Some(String::from("Somewhere")),
                ..Default::default()
            },
            &Default::default(),
            // NOTE: guarded on the etag the read just returned, which
            // exercises the If-Match path against the live API.
            fetched.etag.as_deref(),
        )
    })
    .response;
    assert_eq!(patched.location.as_deref(), Some("Somewhere"));
    assert_eq!(
        patched.summary.as_deref(),
        Some("io-gcal test event"),
        "a patch leaves the fields it does not carry alone"
    );

    let mut replacement = patched.clone();
    replacement.summary = Some(String::from("io-gcal test event renamed"));
    let updated = retry("event update", || {
        client.event_update(
            calendar_id,
            &event_id,
            &replacement,
            &Default::default(),
            None,
        )
    })
    .response;
    assert_eq!(
        updated.summary.as_deref(),
        Some("io-gcal test event renamed")
    );

    let ical_uid = format!("{summary}@pimalaya.org");
    let imported = retry("event import", || {
        client.event_import(
            calendar_id,
            &GcalEvent {
                ical_uid: Some(ical_uid.clone()),
                summary: Some(String::from("io-gcal test import")),
                start: Some(timed("2030-01-07T10:00:00+01:00")),
                end: Some(timed("2030-01-07T11:00:00+01:00")),
                ..Default::default()
            },
            &Default::default(),
        )
    })
    .response;
    assert_eq!(
        imported.ical_uid.as_deref(),
        Some(ical_uid.as_str()),
        "an import keeps the iCalUID it is given"
    );

    let series = retry("recurring event insert", || {
        client.event_insert(
            calendar_id,
            &GcalEvent {
                summary: Some(String::from("io-gcal test series")),
                // NOTE: a timed recurring event needs its start and end
                // anchored in a named time zone, since that is what the
                // recurrence is expanded in; a UTC offset alone is
                // rejected.
                start: Some(timed_in("2030-02-04T10:00:00+01:00", "Europe/Paris")),
                end: Some(timed_in("2030-02-04T10:30:00+01:00", "Europe/Paris")),
                recurrence: vec![String::from("RRULE:FREQ=WEEKLY;COUNT=3")],
                ..Default::default()
            },
            &Default::default(),
        )
    })
    .response;
    let series_id = series.id.clone().expect("the new series carries an id");

    let instances = retry("event instances", || {
        client.event_instances(
            calendar_id,
            &series_id,
            &GcalEventInstancesParams::default(),
        )
    })
    .response;
    assert_eq!(instances.items.len(), 3, "COUNT=3 expands to three");

    let quick = retry("event quick add", || {
        client.event_quick_add(calendar_id, "io-gcal quick event on 2030-03-05 10am", None)
    })
    .response;
    assert!(quick.id.is_some());

    let baseline = retry("events list", || {
        client.events_list(
            calendar_id,
            &GcalEventsListParams {
                single_events: true,
                max_results: Some(50),
                ..Default::default()
            },
        )
    })
    .response;
    assert!(
        baseline.items.len() >= 5,
        "got {} events",
        baseline.items.len()
    );
    let sync_token = baseline
        .next_sync_token
        .clone()
        .expect("the last page carries a sync token");

    retry("event delete", || {
        client.event_delete(calendar_id, &event_id, None, None)
    });

    let changed = retry("incremental events list", || {
        client.events_list(
            calendar_id,
            &GcalEventsListParams {
                single_events: true,
                sync_token: Some(&sync_token),
                ..Default::default()
            },
        )
    })
    .response;
    let deleted = changed
        .items
        .iter()
        .find(|event| event.id.as_deref() == Some(event_id.as_str()))
        .expect("the incremental listing reports the deleted event");
    assert_eq!(deleted.status, Some(GcalEventStatus::Cancelled));
}

/// Moves an event from one calendar the run owns to the other.
fn event_move(client: &mut GcalClientStd, from: &str, to: &str) {
    let event = retry("event insert", || {
        client.event_insert(
            from,
            &GcalEvent {
                summary: Some(String::from("io-gcal test move")),
                start: Some(timed("2030-01-08T10:00:00+01:00")),
                end: Some(timed("2030-01-08T11:00:00+01:00")),
                ..Default::default()
            },
            &Default::default(),
        )
    })
    .response;
    let event_id = event.id.clone().expect("the new event carries an id");

    let moved = retry("event move", || {
        client.event_move(from, &event_id, to, None)
    })
    .response;
    assert_eq!(moved.id.as_deref(), Some(event_id.as_str()));

    let fetched = retry("event get in the destination", || {
        client.event_get(to, &event_id, None, None)
    })
    .response;
    assert_eq!(fetched.summary.as_deref(), Some("io-gcal test move"));
}

/// Adds a calendar shared with the principal to its calendar list,
/// replaces the entry, then removes it.
fn calendar_list(client: &mut GcalClientStd, calendar_id: &str) {
    let inserted = retry("calendar list entry insert", || {
        client.calendar_list_entry_insert(
            &GcalCalendarListEntry {
                id: Some(calendar_id.to_owned()),
                ..Default::default()
            },
            None,
        )
    })
    .response;
    assert_eq!(inserted.id.as_deref(), Some(calendar_id));
    assert_eq!(inserted.access_role, Some(GcalAccessRole::Reader));

    let updated = retry("calendar list entry update", || {
        client.calendar_list_entry_update(
            calendar_id,
            &GcalCalendarListEntry {
                summary_override: Some(String::from("io-gcal replaced")),
                ..Default::default()
            },
            None,
        )
    })
    .response;
    assert_eq!(
        updated.summary_override.as_deref(),
        Some("io-gcal replaced")
    );

    retry("calendar list entry delete", || {
        client.calendar_list_entry_delete(calendar_id)
    });
}

/// Reads the access control list, adds a rule, rewrites it, then
/// removes it.
fn sharing(client: &mut GcalClientStd, calendar_id: &str) {
    let acl = retry("acl list", || {
        client.acl_list(calendar_id, &Default::default())
    })
    .response;
    assert!(
        acl.items
            .iter()
            .any(|rule| rule.role == Some(GcalAccessRole::Owner)),
        "the owner rule always exists"
    );

    let rule = GcalAclRule {
        role: Some(GcalAccessRole::Reader),
        scope: Some(GcalAclScope {
            scope_type: Some(GcalAclScopeType::Default),
            value: None,
        }),
        ..Default::default()
    };
    let created = retry("acl rule insert", || {
        client.acl_rule_insert(calendar_id, &rule, Some(false))
    })
    .response;
    let rule_id = created.id.clone().expect("the new rule carries an id");

    let fetched = retry("acl rule get", || {
        client.acl_rule_get(calendar_id, &rule_id)
    })
    .response;
    assert_eq!(fetched.role, Some(GcalAccessRole::Reader));

    let patched = retry("acl rule patch", || {
        client.acl_rule_patch(
            calendar_id,
            &rule_id,
            &GcalAclRule {
                role: Some(GcalAccessRole::FreeBusyReader),
                ..Default::default()
            },
            Some(false),
        )
    })
    .response;
    assert_eq!(patched.role, Some(GcalAccessRole::FreeBusyReader));

    let updated = retry("acl rule update", || {
        client.acl_rule_update(calendar_id, &rule_id, &rule, Some(false))
    })
    .response;
    assert_eq!(updated.role, Some(GcalAccessRole::Reader));

    if let Err(err) = client.acl_rule_delete(calendar_id, &rule_id) {
        report_leftover("acl rule", &rule_id, &err);
    }
}

/// Asks for the busy periods of the calendar the run created.
fn free_busy(client: &mut GcalClientStd, calendar_id: &str) {
    let request = GcalFreeBusyRequest {
        time_min: Some(String::from("2030-02-01T00:00:00Z")),
        time_max: Some(String::from("2030-03-01T00:00:00Z")),
        items: vec![GcalFreeBusyRequestItem {
            id: Some(String::from(calendar_id)),
        }],
        ..Default::default()
    };

    let response = retry("free/busy query", || client.free_busy_query(&request)).response;
    let calendar = response
        .calendars
        .get(calendar_id)
        .expect("the answer covers the queried calendar");
    assert!(calendar.errors.is_empty(), "got: {:?}", calendar.errors);
    assert!(
        !calendar.busy.is_empty(),
        "the recurring series busies three slots in February"
    );
}

// --- utils ---------------------------------------------------------------

/// A VCALENDAR whose VEVENT touches every property the projection
/// manages, plus one it stashes.
#[cfg(feature = "ical")]
fn ical_document(uid: &str, summary: &str, location: bool) -> String {
    let mut lines = vec![
        String::from("BEGIN:VCALENDAR"),
        String::from("VERSION:2.0"),
        String::from("PRODID:-//pimalaya//io-gcal tests//EN"),
        String::from("BEGIN:VEVENT"),
        format!("UID:{uid}"),
        String::from("DTSTAMP:20300101T000000Z"),
        String::from("DTSTART;TZID=Europe/Paris:20300110T100000"),
        String::from("DTEND;TZID=Europe/Paris:20300110T110000"),
        String::from("RRULE:FREQ=WEEKLY;COUNT=2"),
        format!("SUMMARY:{summary}"),
        String::from("DESCRIPTION:written by the io-gcal suite"),
        String::from("TRANSP:TRANSPARENT"),
        String::from("CLASS:PRIVATE"),
        String::from("X-PIMALAYA-TEST:kept verbatim"),
    ];

    if location {
        lines.push(String::from("LOCATION:Paris"));
    }

    lines.extend([
        String::from("BEGIN:VALARM"),
        String::from("ACTION:DISPLAY"),
        String::from("DESCRIPTION:reminder"),
        String::from("TRIGGER:-PT15M"),
        String::from("END:VALARM"),
        String::from("END:VEVENT"),
        String::from("END:VCALENDAR"),
        String::new(),
    ]);

    lines.join("\r\n")
}

/// Asserts that the event read back projects like the one written, on
/// every field the projection manages.
#[cfg(feature = "ical")]
fn assert_same_projection(
    written: &GcalEvent,
    read: &GcalEvent,
    written_doc: &str,
    read_doc: &str,
) {
    let context = format!("written:\n{written_doc}\nread:\n{read_doc}");

    assert_eq!(read.ical_uid, written.ical_uid, "UID altered\n{context}");
    assert_eq!(read.summary, written.summary, "SUMMARY altered\n{context}");
    assert_eq!(
        read.description, written.description,
        "DESCRIPTION altered\n{context}"
    );
    assert_eq!(
        read.location, written.location,
        "LOCATION altered\n{context}"
    );
    assert_eq!(read.start, written.start, "DTSTART altered\n{context}");
    assert_eq!(read.end, written.end, "DTEND altered\n{context}");
    assert_eq!(
        read.recurrence, written.recurrence,
        "RRULE altered\n{context}"
    );
    assert_eq!(
        read.transparency, written.transparency,
        "TRANSP altered\n{context}"
    );
    assert_eq!(
        read.visibility, written.visibility,
        "CLASS altered\n{context}"
    );
    assert_eq!(
        read.reminders, written.reminders,
        "VALARM altered\n{context}"
    );
    assert_eq!(
        read.extended_properties, written.extended_properties,
        "stash altered\n{context}"
    );
}

/// Whom the run acts as.
enum Principal {
    /// The service account itself, on the calendars it owns.
    ServiceAccount,
    /// The Workspace user the service account acts for.
    Delegated,
}

/// Opens the TCP and TLS connection out of the environment token.
fn connect(principal: Principal) -> GcalClientStd {
    env_logger::try_init().ok();

    GcalClientStd::connect(token(principal), GcalClientStdConnectOptions::default())
        .expect("connect")
}

/// The bearer token the run authenticates with.
///
/// `GCAL_ACCESS_TOKEN` short-circuits everything when it is set, for
/// the token you minted by hand. Otherwise a service account key, held
/// inline in `GCAL_SERVICE_ACCOUNT_KEY` or at the path
/// `GCAL_SERVICE_ACCOUNT_KEY_FILE`, is traded for a fresh token, acting
/// as `GCAL_SERVICE_ACCOUNT_SUBJECT` for a delegated principal.
fn token(principal: Principal) -> String {
    if let Ok(token) = env::var("GCAL_ACCESS_TOKEN") {
        return token;
    }

    let Some(key) = service_account_key() else {
        panic!(
            "set GCAL_ACCESS_TOKEN, or GCAL_SERVICE_ACCOUNT_KEY / \
             GCAL_SERVICE_ACCOUNT_KEY_FILE to mint one"
        );
    };

    match principal {
        Principal::ServiceAccount => mint_token(&key, None),
        Principal::Delegated => mint_token(&key, Some(subject())),
    }
}

/// The Workspace user a delegated principal acts as.
fn subject() -> String {
    env::var("GCAL_SERVICE_ACCOUNT_SUBJECT").unwrap_or_else(|_| String::from(DEFAULT_SUBJECT))
}

/// The service account key, inline or read from its path, unless
/// `GCAL_ACCESS_TOKEN` takes precedence.
fn service_account_key() -> Option<String> {
    if env::var("GCAL_ACCESS_TOKEN").is_ok() {
        return None;
    }

    if let Some(key) = env::var("GCAL_SERVICE_ACCOUNT_KEY")
        .ok()
        .filter(|key| !key.is_empty())
    {
        return Some(key);
    }

    let path = env::var("GCAL_SERVICE_ACCOUNT_KEY_FILE").ok()?;
    let key = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("cannot read the service account key at {path}: {err}"));

    Some(key)
}

/// The subset of a service account key file the JWT bearer grant needs.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct ServiceAccountKey {
    client_email: String,
    private_key: String,
    #[serde(default = "default_token_uri")]
    token_uri: String,
}

fn default_token_uri() -> String {
    String::from("https://oauth2.googleapis.com/token")
}

/// Signs a JWT bearer assertion with the service account key and trades
/// it for an access token (RFC 7523 section 2.1), acting as `subject`
/// when one is given and as the service account itself otherwise.
///
/// This is the grant that needs no human: the assertion *is* the
/// authorization, so there is no consent screen and no refresh token to
/// keep alive. The scopes ride in the claims, which is Google's
/// deviation from the RFC, and io-oauth models it: the token endpoint
/// reads them from there rather than from the request body.
fn mint_token(key: &str, subject: Option<String>) -> String {
    let key: ServiceAccountKey =
        serde_json::from_str(key).expect("the service account key is valid JSON");

    let signer = Oauth20JwtBearerKey::from_pkcs8_pem(&key.private_key)
        .expect("the service account key holds a PKCS#8 private key");

    let token_uri: Url = key.token_uri.parse().expect("the token URI is a valid URL");

    let mut client =
        Oauth20ClientStd::connect(token_uri, &Tls::default(), key.client_email.as_str())
            .expect("connect to the token endpoint");

    let claims = Oauth20JwtBearerClaims {
        iss: key.client_email.as_str().into(),
        sub: subject.map(Cow::from),
        scope: [Cow::from(CALENDAR_SCOPE)].into_iter().collect(),
        ..Default::default()
    };

    // NOTE: iat and exp come from the clock here, in the std client;
    // the coroutine layer underneath stays clock-free.
    let assertion = client
        .sign_jwt_bearer_assertion(&signer, claims, None, Duration::from_secs(600))
        .expect("sign the assertion");

    let params = Oauth20JwtBearerGrantRequestParams {
        assertion,
        scope: Default::default(),
    };

    let response = client
        .request_jwt_bearer_grant(params)
        .expect("trade the assertion for an access token");

    match response {
        Ok(granted) => granted.access_token.expose_secret().to_owned(),
        Err(err) => panic!("the token endpoint refused the assertion: {err:?}"),
    }
}

/// A timed event boundary at the given RFC 3339 timestamp.
fn timed(date_time: &str) -> GcalEventDateTime {
    GcalEventDateTime {
        date_time: Some(String::from(date_time)),
        ..Default::default()
    }
}

/// A timed event boundary anchored in a named time zone, as a
/// recurring event requires.
fn timed_in(date_time: &str, time_zone: &str) -> GcalEventDateTime {
    GcalEventDateTime {
        time_zone: Some(String::from(time_zone)),
        ..timed(date_time)
    }
}

/// Milliseconds since the Unix epoch, to mint a unique calendar name
/// per run.
fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

/// Runs `body`, then `cleanup` whichever way `body` went, and only then
/// re-raises a panic `body` may have raised.
///
/// These flows run against a real account. Every step panics on
/// failure, so a teardown written as the last statements of the flow is
/// skipped the moment anything goes wrong, and each failed run leaves a
/// calendar behind for good. Whatever a run created has to be torn down
/// on the failing path too, which is the one where it matters.
fn with_cleanup<B, C>(client: &mut GcalClientStd, body: B, cleanup: C)
where
    B: FnOnce(&mut GcalClientStd),
    C: FnOnce(&mut GcalClientStd),
{
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| body(client)));

    if panic::catch_unwind(AssertUnwindSafe(|| cleanup(client))).is_err() {
        eprintln!("WARNING: cleanup itself failed, the account may hold leftovers");
    }

    if let Err(payload) = outcome {
        panic::resume_unwind(payload);
    }
}

/// Reports a failed teardown without panicking, naming what was left
/// behind so it can be removed by hand.
fn report_leftover(what: &str, id: &str, err: &dyn Debug) {
    eprintln!("WARNING: could not clean up {what} `{id}`, remove it by hand: {err:?}");
}

/// Runs `call`, retrying with exponential backoff while Google answers
/// that a rate limit is exceeded, and panics with `what` otherwise.
///
/// Calendar signals a quota burst with a 403 rather than a 429, which
/// [`GcalSendError::is_retryable`] does not count, and asks for an
/// exponential backoff. A run issues dozens of writes on one principal
/// within seconds, enough to trip the per-user limit.
///
/// [`GcalSendError::is_retryable`]: io_gcal::v3::send::GcalSendError::is_retryable
fn retry<T>(what: &str, mut call: impl FnMut() -> Result<T, GcalClientStdError>) -> T {
    let mut delay = Duration::from_secs(2);

    for attempt in 1..=6 {
        match call() {
            Ok(out) => return out,
            Err(err) if attempt < 6 && rate_limited(&err) => {
                eprintln!("{what}: rate limited, retrying in {delay:?}");
                thread::sleep(delay);
                delay *= 2;
            }
            Err(err) => panic!("{what}: {err:?}"),
        }
    }

    unreachable!("the last attempt either returns or panics")
}

/// Whether Google refused the call for a rate limit: a transient status,
/// or the 403 Calendar answers a quota burst with.
fn rate_limited(err: &GcalClientStdError) -> bool {
    let GcalClientStdError::Send(err) = err else {
        return false;
    };

    err.is_retryable()
        || (err.status() == Some(403) && {
            let message = err.to_string().to_lowercase();
            message.contains("rate limit") || message.contains("usage limits")
        })
}
