//! Get an event (`events.get`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/events/get>

use alloc::string::ToString;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        rest::events::GcalEvent,
        send::{GcalSend, GcalSendError, GcalSendOutput, gcal_api_url},
    },
};

/// I/O-free coroutine getting an event by id (`events.get`).
pub struct GcalEventGet {
    send: GcalSend<GcalEvent>,
}

impl GcalEventGet {
    /// Builds the `events.get` request for the given calendar and event
    /// id.
    ///
    /// `max_attendees` caps how many attendees come back, and
    /// `time_zone` overrides the time zone the times are expressed in.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        event_id: &str,
        max_attendees: Option<u32>,
        time_zone: Option<&str>,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar event retrieval");
        trace!("calendar_id: {calendar_id:?}");
        trace!("event_id: {event_id:?}");

        let mut url = gcal_api_url(&["calendars", calendar_id, "events", event_id])?;

        if let Some(max_attendees) = max_attendees {
            url.query_pairs_mut()
                .append_pair("maxAttendees", &max_attendees.to_string());
        }

        if let Some(time_zone) = time_zone {
            url.query_pairs_mut().append_pair("timeZone", time_zone);
        }

        let send = GcalSend::get(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalEventGet {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalEvent>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("event retrieved");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
