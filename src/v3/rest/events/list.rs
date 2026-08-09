//! List the events of a calendar (`events.list`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/events/list>

use alloc::{format, string::String};

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use serde::Serialize;
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        query::{append_query_pairs, is_false},
        rest::events::{GcalEventType, GcalEvents},
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// Query parameters for listing events (`events.list`).
///
/// The same set drives [`crate::v3::rest::events::watch`].
///
/// Two of them shape the answer more than the others:
/// `single_events` expands recurring series into their instances rather
/// than returning the series, and `sync_token` turns the listing into
/// an incremental one. A sync token is only compatible with the
/// parameters the previous listing used, and the API rejects it
/// alongside most filters.
#[derive(Debug, Clone, Default, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalEventsListParams<'a> {
    /// Free-text search over the summary, description, location,
    /// attendees, organizer and working location of the events.
    pub q: Option<&'a str>,
    /// The event types to return. Defaults to every type.
    pub event_types: &'a [GcalEventType],
    /// The RFC 5545 unique identifier the returned events must carry.
    #[serde(rename = "iCalUID")]
    pub ical_uid: Option<&'a str>,
    /// The lower bound, exclusive, on the end time of the returned
    /// events, as an RFC 3339 timestamp.
    pub time_min: Option<&'a str>,
    /// The upper bound, exclusive, on the start time of the returned
    /// events, as an RFC 3339 timestamp.
    pub time_max: Option<&'a str>,
    /// The lower bound on the last modification time of the returned
    /// events, as an RFC 3339 timestamp. Setting it also returns the
    /// deleted events.
    pub updated_min: Option<&'a str>,
    /// The time zone the returned times are expressed in, defaulting to
    /// the time zone of the calendar.
    pub time_zone: Option<&'a str>,
    /// The maximum number of attendees to return per event; only the
    /// participant is returned when the event has more.
    pub max_attendees: Option<u32>,
    /// The maximum number of events to return per page, 250 by default
    /// and 2500 at most.
    pub max_results: Option<u32>,
    /// The order of the returned events, unspecified by default.
    pub order_by: Option<GcalEventsOrderBy>,
    /// The page token from a previous list response.
    pub page_token: Option<&'a str>,
    /// The private extended properties the returned events must carry,
    /// each given as `key=value`.
    pub private_extended_property: &'a [String],
    /// The shared extended properties the returned events must carry,
    /// each given as `key=value`.
    pub shared_extended_property: &'a [String],
    /// Whether to expand recurring events into their instances rather
    /// than returning the underlying series.
    #[serde(skip_serializing_if = "is_false")]
    pub single_events: bool,
    /// Whether to include the deleted events and the cancelled
    /// instances of a recurring series.
    #[serde(skip_serializing_if = "is_false")]
    pub show_deleted: bool,
    /// Whether to include the invitations the user has not responded
    /// to yet.
    #[serde(skip_serializing_if = "is_false")]
    pub show_hidden_invitations: bool,
    /// The sync token from the last page of a previous listing,
    /// restricting the response to what changed since then. An expired
    /// token surfaces as an HTTP 410, which callers recover from by
    /// re-baselining a full listing.
    ///
    /// The API rejects it alongside `q`, `ical_uid`, `order_by`,
    /// `time_min`, `time_max`, `updated_min`,
    /// `private_extended_property` and `shared_extended_property`, and
    /// every other parameter must match the listing that produced the
    /// token. Deleted events always come back with it, whatever
    /// `show_deleted` says.
    pub sync_token: Option<&'a str>,
}

/// The order of the events returned by a listing.
#[derive(Debug, Clone, Copy, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GcalEventsOrderBy {
    /// By start time, ascending. Only available when recurring events
    /// are expanded into single instances.
    StartTime,
    /// By last modification time, ascending.
    Updated,
}

/// I/O-free coroutine listing the events of a calendar
/// (`events.list`).
pub struct GcalEventsList {
    send: GcalSend<GcalEvents>,
}

impl GcalEventsList {
    /// Builds the `events.list` request from the given query
    /// parameters.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        params: &GcalEventsListParams,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar events listing");
        trace!("calendar_id: {calendar_id:?}");
        trace!("params: {params:?}");

        let mut url =
            Url::parse(GCAL_API_BASE)?.join(&format!("calendars/{calendar_id}/events"))?;
        append_query_pairs(&mut url, params);

        let send = GcalSend::get(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalEventsList {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalEvents>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("events listed");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
