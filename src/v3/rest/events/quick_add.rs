//! Create an event from a piece of text (`events.quickAdd`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/events/quickAdd>

use alloc::format;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        query::to_field_pairs,
        rest::events::{GcalEvent, GcalSendUpdates},
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// I/O-free coroutine creating an event from a piece of text
/// (`events.quickAdd`).
pub struct GcalEventQuickAdd {
    send: GcalSend<GcalEvent>,
}

impl GcalEventQuickAdd {
    /// Builds the `events.quickAdd` request for the given text, which
    /// the server parses into an event the way the Calendar UI does,
    /// as in `Appointment at Somewhere on June 3rd 10am-10:25am`.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        text: &str,
        send_updates: Option<GcalSendUpdates>,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar event quick add");
        trace!("calendar_id: {calendar_id:?}");
        trace!("text: {text:?}");
        trace!("send_updates: {send_updates:?}");

        if text.trim().is_empty() {
            let err = GcalSendError::InvalidRequest("Quick add text cannot be empty".into());
            return Err(err);
        }

        let mut url =
            Url::parse(GCAL_API_BASE)?.join(&format!("calendars/{calendar_id}/events/quickAdd"))?;

        url.query_pairs_mut().append_pair("text", text);

        url.query_pairs_mut()
            .extend_pairs(to_field_pairs("sendUpdates", &send_updates));

        let send = GcalSend::post_empty(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalEventQuickAdd {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalEvent>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("event quick added");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
