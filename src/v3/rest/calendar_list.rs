//! Calendar list (`calendarList`): list, get, insert, update, patch,
//! delete, watch.
//!
//! The calendar list holds the calendars a given user has subscribed
//! to, and the metadata they layered on top of each one: colour,
//! visibility, notifications, default reminders. The properties shared
//! by every user of a calendar live on the
//! [`crate::v3::rest::calendars`] resource instead.
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/calendarList>

use alloc::{string::String, vec::Vec};

use serde::{Deserialize, Serialize};

use crate::v3::rest::{
    acl::GcalAccessRole,
    calendars::GcalConferenceProperties,
    events::{GcalEventReminder, GcalEventReminderMethod},
};

pub mod delete;
pub mod get;
pub mod insert;
pub mod list;
pub mod patch;
pub mod update;
pub mod watch;

/// The calendar list of a user, one page of entries.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalCalendarList {
    /// Type of the collection, always `calendar#calendarList`.
    #[serde(default)]
    pub kind: Option<String>,
    /// ETag of the collection.
    #[serde(default)]
    pub etag: Option<String>,
    /// The entries of the current page.
    #[serde(default)]
    pub items: Vec<GcalCalendarListEntry>,
    /// The token retrieving the next page, absent on the last page.
    #[serde(default)]
    pub next_page_token: Option<String>,
    /// The token retrieving only what changed since this listing,
    /// present on the last page only.
    #[serde(default)]
    pub next_sync_token: Option<String>,
}

/// One calendar as it appears on a user's calendar list.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalCalendarListEntry {
    /// Type of the resource, always `calendar#calendarListEntry`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// ETag of the resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// Identifier of the calendar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Title of the calendar, read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// The title this user gave the calendar, overriding its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_override: Option<String>,
    /// Description of the calendar, read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Geographic location of the calendar as free-form text,
    /// read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// Time zone of the calendar, read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    /// Index-based colour of the calendar, referring to an entry in the
    /// `calendar` palette of [`crate::v3::rest::colors`]. Superseded by
    /// the explicit foreground and background colours.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_id: Option<String>,
    /// Main colour of the calendar in hexadecimal format, such as
    /// `#0088aa`. Writing it requires the `colorRgbFormat` parameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    /// Foreground colour of the calendar in hexadecimal format, such as
    /// `#ffffff`. Writing it requires the `colorRgbFormat` parameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground_color: Option<String>,
    /// Whether the calendar is hidden from the list, only returned when
    /// it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    /// Whether the content of the calendar shows up in the UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    /// The effective access role this user has on the calendar,
    /// read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_role: Option<GcalAccessRole>,
    /// The default reminders this user has on the calendar.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_reminders: Vec<GcalEventReminder>,
    /// The notifications this user receives for the calendar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_settings: Option<GcalCalendarNotificationSettings>,
    /// Whether the calendar is the primary one of this user, read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<bool>,
    /// Whether the entry has been removed from the calendar list,
    /// read-only. Set on the entries an incremental listing returns as
    /// deleted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
    /// Email of the owner of the calendar, set on secondary calendars
    /// only, read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_owner: Option<String>,
    /// Whether the calendar automatically accepts invitations, valid
    /// for resource calendars only, read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_accept_invitations: Option<bool>,
    /// Which types of conferences are allowed on this calendar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conference_properties: Option<GcalConferenceProperties>,
}

/// The notifications a user receives for a calendar.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalCalendarNotificationSettings {
    /// The notifications set on the calendar.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notifications: Vec<GcalCalendarNotification>,
}

/// One notification a user receives for a calendar.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalCalendarNotification {
    /// What the notification is about. Required when adding one.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub notification_type: Option<GcalCalendarNotificationType>,
    /// How the notification is delivered. Required when adding one.
    ///
    /// Calendar notifications share the reminder methods, but only
    /// [`GcalEventReminderMethod::Email`] is accepted here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<GcalEventReminderMethod>,
}

/// What a calendar notification is about.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GcalCalendarNotificationType {
    /// A new event was put on the calendar.
    EventCreation,
    /// An event changed.
    EventChange,
    /// An event was cancelled.
    EventCancellation,
    /// An attendee responded to an event invitation.
    EventResponse,
    /// The morning agenda of the events of the day.
    Agenda,
}
