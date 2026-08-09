//! Offline coverage of the notification channels (`channels`): the
//! stop method and the channel representation every watch shares.

mod common;

use common::*;
use io_gcal::v3::rest::channels::{GcalChannel, GcalChannelType, stop::GcalChannelStop};

#[test]
fn stops_a_channel() {
    let channel = GcalChannel {
        id: Some(String::from("chan-1")),
        resource_id: Some(String::from("res-1")),
        ..Default::default()
    };
    let mut coroutine = GcalChannelStop::new(&auth(), &channel).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &empty_response("204 No Content"));

    ret.unwrap();

    assert_eq!(request_line(&request), "POST /calendar/v3/channels/stop");
    // NOTE: stopping needs both ids, since the channel id alone does
    // not tell the server which resource the channel was opened on.
    assert_eq!(
        request_body(&request),
        r#"{"id":"chan-1","resourceId":"res-1"}"#
    );
}

#[test]
fn serializes_a_whole_channel() {
    let channel = GcalChannel {
        id: Some(String::from("chan-1")),
        channel_type: Some(GcalChannelType::WebHook),
        address: Some(String::from("https://hook.example.org/gcal")),
        token: Some(String::from("shared-secret")),
        payload: Some(true),
        params: [(String::from("ttl"), String::from("3600"))]
            .into_iter()
            .collect(),
        ..Default::default()
    };

    let json = serde_json::to_string(&channel).unwrap();
    assert!(json.contains(r#""type":"web_hook""#), "got: {json}");
    assert!(json.contains(r#""params":{"ttl":"3600"}"#), "got: {json}");

    let reparsed: GcalChannel = serde_json::from_str(&json).unwrap();
    assert_eq!(channel, reparsed);
}

#[test]
fn parses_an_established_channel() {
    let json = r#"{"kind":"api#channel","id":"chan-1","resourceId":"res-1","resourceUri":"https://www.googleapis.com/calendar/v3/calendars/primary/events?alt=json","expiration":"1780000000000"}"#;
    let channel: GcalChannel = serde_json::from_str(json).unwrap();

    assert_eq!(channel.kind.as_deref(), Some("api#channel"));
    assert_eq!(channel.resource_id.as_deref(), Some("res-1"));
    assert_eq!(channel.expiration.as_deref(), Some("1780000000000"));
    assert!(channel.resource_uri.is_some());
}

#[test]
fn accepts_both_spellings_of_the_webhook_type() {
    for json in [r#"{"type":"web_hook"}"#, r#"{"type":"webhook"}"#] {
        let channel: GcalChannel = serde_json::from_str(json).unwrap();
        assert_eq!(
            channel.channel_type,
            Some(GcalChannelType::WebHook),
            "{json}"
        );
    }

    // NOTE: reading accepts both, writing settles on the documented one.
    let channel = GcalChannel {
        channel_type: Some(GcalChannelType::WebHook),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_string(&channel).unwrap(),
        r#"{"type":"web_hook"}"#
    );
}
