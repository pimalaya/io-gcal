//! Free/busy information (`freebusy`): query.
//!
//! The one resource that answers a question rather than exposing a
//! stored entity: given a time interval and a set of calendars, it
//! returns the busy periods of each, without any event detail.
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/freebusy>

use alloc::{collections::BTreeMap, string::String, vec::Vec};

use serde::{Deserialize, Serialize};

pub mod query;

/// The question asked to `freebusy.query`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalFreeBusyRequest {
    /// The start of the queried interval, as an RFC 3339 timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_min: Option<String>,
    /// The end of the queried interval, as an RFC 3339 timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_max: Option<String>,
    /// The time zone of the response, `UTC` by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    /// The maximum number of members a queried group may have before
    /// the API returns an error for it, capped at 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_expansion_max: Option<u32>,
    /// The maximum number of calendars the response may cover, capped
    /// at 50.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar_expansion_max: Option<u32>,
    /// The calendars and groups to query.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<GcalFreeBusyRequestItem>,
}

/// One calendar or group of the free/busy query.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalFreeBusyRequestItem {
    /// The identifier of the calendar or group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// The answer of `freebusy.query`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalFreeBusyResponse {
    /// Type of the resource, always `calendar#freeBusy`.
    #[serde(default)]
    pub kind: Option<String>,
    /// The start of the covered interval.
    #[serde(default)]
    pub time_min: Option<String>,
    /// The end of the covered interval.
    #[serde(default)]
    pub time_max: Option<String>,
    /// The free/busy information of each queried calendar, keyed by
    /// calendar identifier.
    #[serde(default)]
    pub calendars: BTreeMap<String, GcalFreeBusyCalendar>,
    /// The expansion of each queried group, keyed by group identifier.
    #[serde(default)]
    pub groups: BTreeMap<String, GcalFreeBusyGroup>,
}

/// The free/busy information of one calendar.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalFreeBusyCalendar {
    /// The periods during which the calendar is busy.
    #[serde(default)]
    pub busy: Vec<GcalTimePeriod>,
    /// The errors that made the computation fail for this calendar.
    #[serde(default)]
    pub errors: Vec<GcalError>,
}

/// The expansion of one queried group into its calendars.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalFreeBusyGroup {
    /// The identifiers of the calendars within the group.
    #[serde(default)]
    pub calendars: Vec<String>,
    /// The errors that made the computation fail for this group.
    #[serde(default)]
    pub errors: Vec<GcalError>,
}

/// A half-open time interval, start inclusive and end exclusive.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalTimePeriod {
    /// The inclusive start of the period, as an RFC 3339 timestamp.
    #[serde(default)]
    pub start: Option<String>,
    /// The exclusive end of the period, as an RFC 3339 timestamp.
    #[serde(default)]
    pub end: Option<String>,
}

/// A per-calendar or per-group error of a free/busy query.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalError {
    /// Broad category of the error.
    #[serde(default)]
    pub domain: Option<String>,
    /// Specific reason of the error, such as `groupTooBig`,
    /// `tooManyCalendarsRequested`, `notFound` or `internalError`.
    #[serde(default)]
    pub reason: Option<String>,
}
