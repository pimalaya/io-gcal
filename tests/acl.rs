//! Offline coverage of the access control rules (`acl`): the seven
//! methods, their query parameters and the rule representation.

mod common;

use common::*;
use io_gcal::v3::rest::{
    acl::{
        GcalAccessRole, GcalAclRule, GcalAclScope, GcalAclScopeType,
        delete::GcalAclRuleDelete,
        get::GcalAclRuleGet,
        insert::GcalAclRuleInsert,
        list::{GcalAclList, GcalAclListParams},
        patch::GcalAclRulePatch,
        update::GcalAclRuleUpdate,
        watch::GcalAclWatch,
    },
    channels::{GcalChannel, GcalChannelType},
};

const RULE: &str = r#"{"kind":"calendar#aclRule","etag":"\"tag-1\"","id":"user:jane@example.org","role":"writer","scope":{"type":"user","value":"jane@example.org"}}"#;

fn rule() -> GcalAclRule {
    GcalAclRule {
        role: Some(GcalAccessRole::Reader),
        scope: Some(GcalAclScope {
            scope_type: Some(GcalAclScopeType::User),
            value: Some(String::from("jane@example.org")),
        }),
        ..Default::default()
    }
}

#[test]
fn lists_the_rules_of_a_calendar() {
    let body = format!(
        r#"{{"kind":"calendar#acl","etag":"\"col\"","items":[{RULE}],"nextSyncToken":"sync-1"}}"#
    );
    let params = GcalAclListParams {
        max_results: Some(10),
        show_deleted: true,
        page_token: Some("page-2"),
        sync_token: Some("sync-0"),
    };
    let mut coroutine = GcalAclList::new(&auth(), "primary", &params).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", &body));
    let out = ret.unwrap();

    assert_eq!(out.response.items.len(), 1);
    assert_eq!(out.response.items[0].role, Some(GcalAccessRole::Writer));
    assert_eq!(
        out.response.items[0]
            .scope
            .as_ref()
            .and_then(|scope| scope.value.as_deref()),
        Some("jane@example.org")
    );
    assert_eq!(out.response.next_sync_token.as_deref(), Some("sync-1"));

    assert!(
        request_line(&request).starts_with("GET /calendar/v3/calendars/primary/acl?"),
        "got: {request}"
    );
    assert_query(&request, "maxResults=10");
    assert_query(&request, "showDeleted=true");
    assert_query(&request, "pageToken=page-2");
    assert_query(&request, "syncToken=sync-0");
}

#[test]
fn lists_the_rules_without_any_parameter() {
    let mut coroutine = GcalAclList::new(&auth(), "primary", &Default::default()).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", "{}"));
    let out = ret.unwrap();

    assert!(out.response.items.is_empty());

    assert_eq!(
        request_line(&request),
        "GET /calendar/v3/calendars/primary/acl"
    );
    // NOTE: a false flag stays off the wire entirely rather than being
    // sent as `showDeleted=false`, which some methods reject.
    assert_no_query(&request, "showDeleted");
}

#[test]
fn escapes_the_calendar_id_of_a_listing() {
    let mut coroutine = GcalAclList::new(
        &auth(),
        "team@group.calendar.google.com",
        &Default::default(),
    )
    .unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", "{}"));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "GET /calendar/v3/calendars/team@group.calendar.google.com/acl"
    );
}

#[test]
fn gets_a_rule() {
    let mut coroutine = GcalAclRuleGet::new(&auth(), "primary", "user:jane@example.org").unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", RULE));
    let out = ret.unwrap();

    assert_eq!(out.response.id.as_deref(), Some("user:jane@example.org"));
    assert_eq!(out.response.etag.as_deref(), Some("\"tag-1\""));

    assert_eq!(
        request_line(&request),
        "GET /calendar/v3/calendars/primary/acl/user:jane@example.org"
    );
}

#[test]
fn inserts_a_rule() {
    let mut coroutine = GcalAclRuleInsert::new(&auth(), "primary", &rule(), None).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", RULE));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "POST /calendar/v3/calendars/primary/acl"
    );
    assert!(
        request.contains("Content-Type: application/json"),
        "got: {request}"
    );
    assert_eq!(
        request_body(&request),
        r#"{"scope":{"type":"user","value":"jane@example.org"},"role":"reader"}"#
    );
}

#[test]
fn inserts_a_rule_without_notifying_the_grantee() {
    let mut coroutine = GcalAclRuleInsert::new(&auth(), "primary", &rule(), Some(false)).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", RULE));

    ret.unwrap();

    assert_query(&request, "sendNotifications=false");
}

#[test]
fn updates_a_rule() {
    let mut coroutine = GcalAclRuleUpdate::new(
        &auth(),
        "primary",
        "user:jane@example.org",
        &rule(),
        Some(true),
    )
    .unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", RULE));

    ret.unwrap();

    assert!(
        request_line(&request)
            .starts_with("PUT /calendar/v3/calendars/primary/acl/user:jane@example.org?"),
        "got: {request}"
    );
    assert_query(&request, "sendNotifications=true");
}

#[test]
fn patches_a_rule() {
    let patch = GcalAclRule {
        role: Some(GcalAccessRole::FreeBusyReader),
        ..Default::default()
    };
    let mut coroutine =
        GcalAclRulePatch::new(&auth(), "primary", "user:jane@example.org", &patch, None).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", RULE));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "PATCH /calendar/v3/calendars/primary/acl/user:jane@example.org"
    );
    // NOTE: a patch only carries what it changes, so the untouched
    // scope must not appear in the body.
    assert_eq!(request_body(&request), r#"{"role":"freeBusyReader"}"#);
}

#[test]
fn patches_a_rule_notifying_the_grantee() {
    let mut coroutine = GcalAclRulePatch::new(
        &auth(),
        "primary",
        "user:jane@example.org",
        &rule(),
        Some(true),
    )
    .unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", RULE));

    ret.unwrap();

    assert_query(&request, "sendNotifications=true");
}

#[test]
fn deletes_a_rule() {
    let mut coroutine =
        GcalAclRuleDelete::new(&auth(), "primary", "user:jane@example.org").unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &empty_response("204 No Content"));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "DELETE /calendar/v3/calendars/primary/acl/user:jane@example.org"
    );
}

#[test]
fn watches_the_rules_of_a_calendar() {
    let channel = GcalChannel {
        id: Some(String::from("chan-1")),
        channel_type: Some(GcalChannelType::WebHook),
        address: Some(String::from("https://hook.example.org/gcal")),
        ..Default::default()
    };
    let params = GcalAclListParams {
        max_results: Some(5),
        ..Default::default()
    };
    let response = json_response(
        "200 OK",
        r#"{"kind":"api#channel","id":"chan-1","resourceId":"res-1","expiration":"1780000000000"}"#,
    );
    let mut coroutine = GcalAclWatch::new(&auth(), "primary", &channel, &params).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &response);
    let out = ret.unwrap();

    assert_eq!(out.response.resource_id.as_deref(), Some("res-1"));
    assert_eq!(out.response.expiration.as_deref(), Some("1780000000000"));

    assert!(
        request_line(&request).starts_with("POST /calendar/v3/calendars/primary/acl/watch?"),
        "got: {request}"
    );
    assert_query(&request, "maxResults=5");
    assert!(
        request_body(&request).contains(r#""type":"web_hook""#),
        "got: {request}"
    );
}

#[test]
fn spells_out_every_access_role() {
    let roles = [
        ("none", GcalAccessRole::None),
        ("freeBusyReader", GcalAccessRole::FreeBusyReader),
        ("reader", GcalAccessRole::Reader),
        (
            "writerWithoutPrivateAccess",
            GcalAccessRole::WriterWithoutPrivateAccess,
        ),
        ("writer", GcalAccessRole::Writer),
        ("owner", GcalAccessRole::Owner),
    ];

    for (wire, role) in roles {
        let json = format!(r#""{wire}""#);
        assert_eq!(serde_json::to_string(&role).unwrap(), json);
        assert_eq!(
            serde_json::from_str::<GcalAccessRole>(&json).unwrap(),
            role,
            "{wire}"
        );
    }
}

#[test]
fn spells_out_every_scope_type() {
    let types = [
        ("default", GcalAclScopeType::Default),
        ("user", GcalAclScopeType::User),
        ("group", GcalAclScopeType::Group),
        ("domain", GcalAclScopeType::Domain),
    ];

    for (wire, scope_type) in types {
        let json = format!(r#""{wire}""#);
        assert_eq!(serde_json::to_string(&scope_type).unwrap(), json);
        assert_eq!(
            serde_json::from_str::<GcalAclScopeType>(&json).unwrap(),
            scope_type,
            "{wire}"
        );
    }
}
