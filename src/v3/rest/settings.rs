//! User settings (`settings`): list, get, watch.
//!
//! Settings are the read-only key/value preferences of the
//! authenticated user, such as their default time zone or the first day
//! of their week.
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/settings>

use alloc::{string::String, vec::Vec};

use serde::{Deserialize, Serialize};

pub mod get;
pub mod list;
pub mod watch;

/// The settings of a user, one page of entries.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalSettings {
    /// Type of the collection, always `calendar#settings`.
    #[serde(default)]
    pub kind: Option<String>,
    /// ETag of the collection.
    #[serde(default)]
    pub etag: Option<String>,
    /// The settings of the current page.
    #[serde(default)]
    pub items: Vec<GcalSetting>,
    /// The token retrieving the next page, absent on the last page.
    #[serde(default)]
    pub next_page_token: Option<String>,
    /// The token retrieving only what changed since this listing,
    /// present on the last page only.
    #[serde(default)]
    pub next_sync_token: Option<String>,
}

/// One user setting, as an id and its value.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalSetting {
    /// Type of the resource, always `calendar#setting`.
    #[serde(default)]
    pub kind: Option<String>,
    /// ETag of the resource.
    #[serde(default)]
    pub etag: Option<String>,
    /// The id of the setting, such as `timezone` or `weekStart`.
    #[serde(default)]
    pub id: Option<String>,
    /// The value of the setting, whose format depends on the id. Always
    /// a UTF-8 string of at most 1024 characters.
    #[serde(default)]
    pub value: Option<String>,
}
