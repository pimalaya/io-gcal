//! Move an event to another calendar (`events.move`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/events/move>

use alloc::format;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use serde_variant::to_variant_name;
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        rest::events::{GcalEvent, GcalSendUpdates},
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// I/O-free coroutine moving an event to another calendar
/// (`events.move`).
pub struct GcalEventMove {
    send: GcalSend<GcalEvent>,
}

impl GcalEventMove {
    /// Builds the `events.move` request for the given event and
    /// destination calendar.
    ///
    /// Only a single, non-recurring event can be moved, and both
    /// calendars must have the same organizer.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        event_id: &str,
        destination: &str,
        send_updates: Option<GcalSendUpdates>,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar event move");
        trace!("calendar_id: {calendar_id:?}");
        trace!("event_id: {event_id:?}");
        trace!("destination: {destination:?}");
        trace!("send_updates: {send_updates:?}");

        let mut url = Url::parse(GCAL_API_BASE)?
            .join(&format!("calendars/{calendar_id}/events/{event_id}/move"))?;

        url.query_pairs_mut()
            .append_pair("destination", destination);

        if let Some(send_updates) = send_updates {
            url.query_pairs_mut().append_pair(
                "sendUpdates",
                to_variant_name(&send_updates).unwrap_or_default(),
            );
        }

        let send = GcalSend::post_empty(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalEventMove {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalEvent>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("event moved");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
