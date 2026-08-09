//! Delete an event (`events.delete`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/events/delete>

use alloc::format;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use serde_variant::to_variant_name;
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        rest::events::GcalSendUpdates,
        send::{GCAL_API_BASE, GcalNoResponse, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// I/O-free coroutine deleting an event by id (`events.delete`).
pub struct GcalEventDelete {
    send: GcalSend<GcalNoResponse>,
}

impl GcalEventDelete {
    /// Builds the `events.delete` request for the given calendar and
    /// event id.
    ///
    /// `send_updates` chooses who gets notified of the cancellation by
    /// email, and `if_match` gates the deletion on the etag a read
    /// returned, so an event that changed since comes back as HTTP 412
    /// rather than vanishing.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        event_id: &str,
        send_updates: Option<GcalSendUpdates>,
        if_match: Option<&str>,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar event deletion");
        trace!("calendar_id: {calendar_id:?}");
        trace!("event_id: {event_id:?}");
        trace!("send_updates: {send_updates:?}");

        let mut url = Url::parse(GCAL_API_BASE)?
            .join(&format!("calendars/{calendar_id}/events/{event_id}"))?;

        if let Some(send_updates) = send_updates {
            url.query_pairs_mut().append_pair(
                "sendUpdates",
                to_variant_name(&send_updates).unwrap_or_default(),
            );
        }

        let send = GcalSend::delete_if_match(auth, url, if_match);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalEventDelete {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalNoResponse>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("event deleted");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
