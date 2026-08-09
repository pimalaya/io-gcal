//! Access control rules (`acl`): list, get, insert, update, patch,
//! delete, watch.
//!
//! An ACL rule grants a scope (a user, a group, a domain or everyone) a
//! role on one calendar.
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/acl>

use alloc::{string::String, vec::Vec};

use serde::{Deserialize, Serialize};

pub mod delete;
pub mod get;
pub mod insert;
pub mod list;
pub mod patch;
pub mod update;
pub mod watch;

/// The access control list of a calendar, one page of rules.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalAcl {
    /// Type of the collection, always `calendar#acl`.
    #[serde(default)]
    pub kind: Option<String>,
    /// ETag of the collection.
    #[serde(default)]
    pub etag: Option<String>,
    /// The rules of the current page.
    #[serde(default)]
    pub items: Vec<GcalAclRule>,
    /// The token retrieving the next page, absent on the last page.
    #[serde(default)]
    pub next_page_token: Option<String>,
    /// The token retrieving only what changed since this listing,
    /// present on the last page only.
    #[serde(default)]
    pub next_sync_token: Option<String>,
}

/// A rule granting one scope one role on a calendar.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalAclRule {
    /// Type of the resource, always `calendar#aclRule`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// ETag of the resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// Identifier of the rule, of the form `<scope type>:<scope value>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The extent to which calendar access is granted by this rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<GcalAclScope>,
    /// The role assigned to the scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<GcalAccessRole>,
}

/// The extent to which calendar access is granted by an ACL rule.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalAclScope {
    /// The type of the scope.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub scope_type: Option<GcalAclScopeType>,
    /// The email address of a user or group, or the name of a domain,
    /// depending on the scope type. Omitted for the default scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// The kind of grantee an ACL rule scope designates.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GcalAclScopeType {
    /// The public scope, granting the role to everyone.
    Default,
    /// A single user, identified by email address.
    User,
    /// A group, identified by email address.
    Group,
    /// A whole domain, identified by name.
    Domain,
}

/// The level of access a user has on a calendar.
///
/// The same ladder backs the role of an ACL rule, the effective access
/// role of a calendar list entry and the `minAccessRole` listing
/// filter. [`GcalAccessRole::None`] only ever appears on an ACL rule
/// and on the access role of an event listing, and is rejected as a
/// `minAccessRole`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GcalAccessRole {
    /// No access at all.
    None,
    /// Read access to free/busy information only.
    FreeBusyReader,
    /// Read access to the calendar, with private event details hidden.
    Reader,
    /// Read and write access, with private event details hidden.
    WriterWithoutPrivateAccess,
    /// Read and write access, with private event details visible.
    Writer,
    /// Manager access: everything a writer can do, plus modifying the
    /// access levels of other users.
    Owner,
}
