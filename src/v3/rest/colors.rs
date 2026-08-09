//! Colours (`colors`): get.
//!
//! The global palettes an event's `colorId` and a calendar list entry's
//! `colorId` refer to.
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/colors>

use alloc::{collections::BTreeMap, string::String};

use serde::{Deserialize, Serialize};

pub mod get;

/// The global colour palettes of the Calendar API.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalColors {
    /// Type of the resource, always `calendar#colors`.
    #[serde(default)]
    pub kind: Option<String>,
    /// Last modification time of the palettes, as an RFC 3339
    /// timestamp.
    #[serde(default)]
    pub updated: Option<String>,
    /// The event palette, keyed by the colour id an event's `colorId`
    /// carries.
    #[serde(default)]
    pub event: BTreeMap<String, GcalColorDefinition>,
    /// The calendar palette, keyed by the colour id a calendar list
    /// entry's `colorId` carries.
    #[serde(default)]
    pub calendar: BTreeMap<String, GcalColorDefinition>,
}

/// One entry of a colour palette.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalColorDefinition {
    /// The background colour of the definition.
    #[serde(default)]
    pub background: Option<String>,
    /// The foreground colour to write on top of the background one.
    #[serde(default)]
    pub foreground: Option<String>,
}
