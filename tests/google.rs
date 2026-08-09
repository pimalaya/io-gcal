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
//! cargo test --test google -- --ignored
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
//! cargo test --test google -- --ignored
//! ```
//!
//! CI passes the key itself rather than a path, as
//! `GCAL_SERVICE_ACCOUNT_KEY`, since it comes straight out of a secret.
//!
//! Both principals are covered on purpose, so the assertions below only
//! state what holds for either. A service account owns its own
//! calendars, which is all these tests touch: no personal calendar is
//! read or written, whichever token is used.
//!
//! [`account`] only reads, and is satisfied by the
//! `calendar.readonly` scope. [`calendar`] walks the whole CRUD
//! surface on a throwaway secondary calendar and needs the full
//! `calendar` scope; everything it creates lives inside that calendar,
//! and [`with_cleanup`] deletes it however the run ends, so a failure
//! leaves the account with one stray calendar at worst.
//!
//! The push channels are deliberately left out: a `watch` needs a
//! publicly reachable HTTPS webhook Google can POST to, which a test
//! process cannot provide.

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
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use io_gcal::v3::{
    client::{GcalClientStd, GcalClientStdConnectOptions},
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

/// Read-only pass over the account-wide resources: the colour
/// palettes, the user settings and the calendar list.
#[test]
#[ignore = "requires GCAL_ACCESS_TOKEN env var and --ignored"]
fn account() {
    let mut client = connect();

    let colors = client.colors_get().expect("colors get").response;
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
    let settings = client
        .settings_list(&Default::default())
        .expect("settings list")
        .response;
    assert!(
        settings
            .items
            .iter()
            .all(|setting| setting.id.is_some() && setting.value.is_some()),
        "every setting carries an id and a value"
    );

    let calendars = client
        .calendar_list_list(&Default::default())
        .expect("calendar list list")
        .response;
    assert!(
        calendars
            .items
            .iter()
            .all(|entry| entry.id.is_some() && entry.access_role.is_some()),
        "every calendar list entry carries an id and an access role"
    );
}

/// Full CRUD pass on a throwaway secondary calendar.
#[test]
#[ignore = "requires GCAL_ACCESS_TOKEN env var and --ignored"]
fn calendar() {
    let mut client = connect();

    let summary = format!("io-gcal-test-{}", unix_millis());
    let created = client
        .calendar_insert(&GcalCalendar {
            summary: Some(summary.clone()),
            time_zone: Some(String::from("Europe/Paris")),
            ..Default::default()
        })
        .expect("calendar insert")
        .response;
    let id = created.id.clone().expect("the new calendar carries an id");

    with_cleanup(
        &mut client,
        |client| {
            calendar_metadata(client, &id, &summary);
            events(client, &id);
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

/// Reads back the calendar, patches it, and checks the entry the
/// creation added to the calendar list.
fn calendar_metadata(client: &mut GcalClientStd, id: &str, summary: &str) {
    let fetched = client.calendar_get(id).expect("calendar get").response;
    assert_eq!(fetched.summary.as_deref(), Some(summary));
    assert_eq!(fetched.time_zone.as_deref(), Some("Europe/Paris"));

    let patched = client
        .calendar_patch(
            id,
            &GcalCalendar {
                description: Some(String::from("patched by the io-gcal suite")),
                ..Default::default()
            },
        )
        .expect("calendar patch")
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

    let entry = client
        .calendar_list_entry_get(id)
        .expect("calendar list entry get")
        .response;
    assert_eq!(entry.access_role, Some(GcalAccessRole::Owner));

    let renamed = client
        .calendar_list_entry_patch(
            id,
            &GcalCalendarListEntry {
                summary_override: Some(String::from("io-gcal override")),
                ..Default::default()
            },
            None,
        )
        .expect("calendar list entry patch")
        .response;
    assert_eq!(
        renamed.summary_override.as_deref(),
        Some("io-gcal override")
    );
}

/// Creates, reads, writes, expands and deletes events, then replays the
/// listing incrementally to see the deletion reported.
fn events(client: &mut GcalClientStd, calendar_id: &str) {
    let event = client
        .event_insert(
            calendar_id,
            &GcalEvent {
                summary: Some(String::from("io-gcal test event")),
                start: Some(timed("2030-01-06T10:00:00+01:00")),
                end: Some(timed("2030-01-06T11:00:00+01:00")),
                ..Default::default()
            },
            &Default::default(),
        )
        .expect("event insert")
        .response;
    let event_id = event.id.clone().expect("the new event carries an id");

    let fetched = client
        .event_get(calendar_id, &event_id, None, None)
        .expect("event get")
        .response;
    assert_eq!(fetched.summary.as_deref(), Some("io-gcal test event"));
    assert!(fetched.ical_uid.is_some(), "the API assigns an iCalUID");

    let patched = client
        .event_patch(
            calendar_id,
            &event_id,
            &GcalEvent {
                location: Some(String::from("Somewhere")),
                ..Default::default()
            },
            &Default::default(),
        )
        .expect("event patch")
        .response;
    assert_eq!(patched.location.as_deref(), Some("Somewhere"));
    assert_eq!(
        patched.summary.as_deref(),
        Some("io-gcal test event"),
        "a patch leaves the fields it does not carry alone"
    );

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

    let series = client
        .event_insert(
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
        .expect("recurring event insert")
        .response;
    let series_id = series.id.clone().expect("the new series carries an id");

    let instances = client
        .event_instances(
            calendar_id,
            &series_id,
            &GcalEventInstancesParams::default(),
        )
        .expect("event instances")
        .response;
    assert_eq!(instances.items.len(), 3, "COUNT=3 expands to three");

    let quick = client
        .event_quick_add(calendar_id, "io-gcal quick event on 2030-03-05 10am", None)
        .expect("event quick add")
        .response;
    assert!(quick.id.is_some());

    let baseline = client
        .events_list(
            calendar_id,
            &GcalEventsListParams {
                single_events: true,
                max_results: Some(50),
                ..Default::default()
            },
        )
        .expect("events list")
        .response;
    assert!(
        baseline.items.len() >= 4,
        "got {} events",
        baseline.items.len()
    );
    let sync_token = baseline
        .next_sync_token
        .clone()
        .expect("the last page carries a sync token");

    client
        .event_delete(calendar_id, &event_id, None)
        .expect("event delete");

    let changed = client
        .events_list(
            calendar_id,
            &GcalEventsListParams {
                single_events: true,
                sync_token: Some(&sync_token),
                ..Default::default()
            },
        )
        .expect("incremental events list")
        .response;
    let deleted = changed
        .items
        .iter()
        .find(|event| event.id.as_deref() == Some(event_id.as_str()))
        .expect("the incremental listing reports the deleted event");
    assert_eq!(deleted.status, Some(GcalEventStatus::Cancelled));
}

/// Reads the access control list, adds a rule, then removes it.
fn sharing(client: &mut GcalClientStd, calendar_id: &str) {
    let acl = client
        .acl_list(calendar_id, &Default::default())
        .expect("acl list")
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
    let created = client
        .acl_rule_insert(calendar_id, &rule, Some(false))
        .expect("acl rule insert")
        .response;
    let rule_id = created.id.clone().expect("the new rule carries an id");

    let fetched = client
        .acl_rule_get(calendar_id, &rule_id)
        .expect("acl rule get")
        .response;
    assert_eq!(fetched.role, Some(GcalAccessRole::Reader));

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

    let response = client
        .free_busy_query(&request)
        .expect("free/busy query")
        .response;
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

/// Opens the TCP and TLS connection out of the environment token.
fn connect() -> GcalClientStd {
    env_logger::try_init().ok();

    GcalClientStd::connect(token(), GcalClientStdConnectOptions::default()).expect("connect")
}

/// The bearer token the run authenticates with.
///
/// `GCAL_ACCESS_TOKEN` short-circuits everything when it is set, for
/// the token you minted by hand. Otherwise a service account key, held
/// inline in `GCAL_SERVICE_ACCOUNT_KEY` or at the path
/// `GCAL_SERVICE_ACCOUNT_KEY_FILE`, is traded for a fresh token.
fn token() -> String {
    if let Ok(token) = env::var("GCAL_ACCESS_TOKEN") {
        return token;
    }

    if let Ok(key) = env::var("GCAL_SERVICE_ACCOUNT_KEY") {
        return mint_token(&key);
    }

    if let Ok(path) = env::var("GCAL_SERVICE_ACCOUNT_KEY_FILE") {
        let key = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("cannot read the service account key at {path}: {err}"));

        return mint_token(&key);
    }

    panic!(
        "set GCAL_ACCESS_TOKEN, or GCAL_SERVICE_ACCOUNT_KEY / \
         GCAL_SERVICE_ACCOUNT_KEY_FILE to mint one"
    );
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
/// it for an access token (RFC 7523 section 2.1).
///
/// This is the grant that needs no human: the assertion *is* the
/// authorization, so there is no consent screen and no refresh token to
/// keep alive. The scopes ride in the claims, which is Google's
/// deviation from the RFC, and io-oauth models it: the token endpoint
/// reads them from there rather than from the request body.
fn mint_token(key: &str) -> String {
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
