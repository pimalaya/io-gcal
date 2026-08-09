//! Notification channels (`channels`): stop.
//!
//! A channel is the push subscription a `watch` method opens on a
//! resource. Every `watch` takes a channel as request body and returns
//! the established one; [`stop`] closes it.
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/channels>

use alloc::{collections::BTreeMap, string::String};

use serde::{Deserialize, Serialize};

pub mod stop;

/// A notification channel watching a Calendar resource for changes.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalChannel {
    /// Type of the resource, always `api#channel`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// A UUID or similar unique string identifying this channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Opaque id of the watched resource, stable across API versions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    /// Version-specific identifier of the watched resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_uri: Option<String>,
    /// Arbitrary string delivered to the target address with every
    /// notification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Expiration of the channel, as a Unix timestamp in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiration: Option<String>,
    /// Delivery mechanism of the channel.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub channel_type: Option<GcalChannelType>,
    /// Address the notifications are delivered to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Whether the notifications carry a payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<bool>,
    /// Additional parameters controlling the delivery behaviour.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
}

/// Delivery mechanism of a notification channel.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
pub enum GcalChannelType {
    /// Notifications are delivered as HTTP requests to the channel
    /// address. The API also accepts the `webhook` spelling, which
    /// deserializes into this same variant.
    #[serde(rename = "web_hook", alias = "webhook")]
    WebHook,
}
