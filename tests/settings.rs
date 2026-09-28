//! Offline coverage of the user settings (`settings`): list, get and
//! watch.

mod common;

use common::*;
use io_gcal::v3::rest::{
    channels::{GcalChannel, GcalChannelType},
    settings::{
        get::GcalSettingGet,
        list::{GcalSettingsList, GcalSettingsListParams},
        watch::GcalSettingsWatch,
    },
};

const SETTINGS: &str = r#"{"kind":"calendar#settings","etag":"\"col\"","items":[{"kind":"calendar#setting","id":"timezone","value":"Europe/Paris"},{"kind":"calendar#setting","id":"weekStart","value":"1"}],"nextSyncToken":"s1"}"#;

#[test]
fn lists_the_settings() {
    let params = GcalSettingsListParams {
        max_results: Some(10),
        page_token: Some("p1"),
        sync_token: Some("s0"),
    };
    let mut coroutine = GcalSettingsList::new(&auth(), &params).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", SETTINGS));
    let out = ret.unwrap();

    assert_eq!(out.response.items.len(), 2);
    assert_eq!(out.response.items[0].id.as_deref(), Some("timezone"));
    assert_eq!(out.response.next_sync_token.as_deref(), Some("s1"));

    assert!(
        request_line(&request).starts_with("GET /calendar/v3/users/me/settings?"),
        "got: {request}"
    );
    assert_query(&request, "maxResults=10");
    assert_query(&request, "pageToken=p1");
    assert_query(&request, "syncToken=s0");
}

#[test]
fn lists_the_settings_without_any_parameter() {
    let mut coroutine = GcalSettingsList::new(&auth(), &Default::default()).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", "{}"));

    ret.unwrap();

    assert_eq!(request_line(&request), "GET /calendar/v3/users/me/settings");
}

#[test]
fn gets_a_setting() {
    let response = r#"{"kind":"calendar#setting","id":"timezone","value":"Europe/Paris"}"#;
    let mut coroutine = GcalSettingGet::new(&auth(), "timezone").unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", response));
    let out = ret.unwrap();

    assert_eq!(out.response.value.as_deref(), Some("Europe/Paris"));

    assert_eq!(
        request_line(&request),
        "GET /calendar/v3/users/me/settings/timezone"
    );
}

#[test]
fn watches_the_settings() {
    let channel = GcalChannel {
        id: Some(String::from("chan-1")),
        channel_type: Some(GcalChannelType::WebHook),
        address: Some(String::from("https://hook.example.org/gcal")),
        ..Default::default()
    };
    let response = json_response("200 OK", r#"{"id":"chan-1","resourceId":"res-1"}"#);
    let mut coroutine = GcalSettingsWatch::new(&auth(), &channel, &Default::default()).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &response);
    let out = ret.unwrap();

    assert_eq!(out.response.resource_id.as_deref(), Some("res-1"));

    assert_eq!(
        request_line(&request),
        "POST /calendar/v3/users/me/settings/watch"
    );
    assert!(
        request_body(&request).contains(r#""address":"https://hook.example.org/gcal""#),
        "got: {request}"
    );
}
