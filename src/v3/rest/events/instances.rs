//! List the instances of a recurring event (`events.instances`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/events/instances>

use alloc::format;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use serde::Serialize;
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        query::{is_false, to_query_pairs},
        rest::events::GcalEvents,
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// Query parameters for listing the instances of a recurring event
/// (`events.instances`).
#[derive(Debug, Clone, Default, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalEventInstancesParams<'a> {
    /// The lower bound, exclusive, on the end time of the returned
    /// instances, as an RFC 3339 timestamp.
    pub time_min: Option<&'a str>,
    /// The upper bound, exclusive, on the start time of the returned
    /// instances, as an RFC 3339 timestamp.
    pub time_max: Option<&'a str>,
    /// The original start time of the one instance to return, for the
    /// series that have moved instances.
    pub original_start: Option<&'a str>,
    /// The time zone the returned times are expressed in, defaulting to
    /// the time zone of the calendar.
    pub time_zone: Option<&'a str>,
    /// The maximum number of attendees to return per instance.
    pub max_attendees: Option<u32>,
    /// The maximum number of instances to return per page, 250 by
    /// default and 2500 at most.
    pub max_results: Option<u32>,
    /// The page token from a previous instances response.
    pub page_token: Option<&'a str>,
    /// Whether to include the cancelled instances of the series.
    #[serde(skip_serializing_if = "is_false")]
    pub show_deleted: bool,
}

/// I/O-free coroutine listing the instances of a recurring event
/// (`events.instances`).
pub struct GcalEventInstances {
    send: GcalSend<GcalEvents>,
}

impl GcalEventInstances {
    /// Builds the `events.instances` request for the given recurring
    /// event.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        event_id: &str,
        params: &GcalEventInstancesParams,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar event instances listing");
        trace!("calendar_id: {calendar_id:?}");
        trace!("event_id: {event_id:?}");
        trace!("params: {params:?}");

        let mut url = Url::parse(GCAL_API_BASE)?.join(&format!(
            "calendars/{calendar_id}/events/{event_id}/instances"
        ))?;
        url.query_pairs_mut().extend_pairs(to_query_pairs(params));

        let send = GcalSend::get(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalEventInstances {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalEvents>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("event instances listed");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
