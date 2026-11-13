//! Watch the events of a calendar for changes (`events.watch`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/events/watch>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        query::append_query_pairs,
        rest::{channels::GcalChannel, events::list::GcalEventsListParams},
        send::{GcalSend, GcalSendError, GcalSendOutput, gcal_api_url},
    },
};

/// I/O-free coroutine opening a notification channel on the events of a
/// calendar (`events.watch`).
pub struct GcalEventsWatch {
    send: GcalSend<GcalChannel>,
}

impl GcalEventsWatch {
    /// Builds the `events.watch` request from the given channel, which
    /// must carry at least its id, its type and its address.
    ///
    /// The params narrow the watched set exactly as they narrow a
    /// listing. A notification carries no payload beyond the fact that
    /// something changed, so the receiver answers it by running the
    /// matching incremental listing.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        channel: &GcalChannel,
        params: &GcalEventsListParams,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar events watch");
        trace!("calendar_id: {calendar_id:?}");
        trace!("channel: {channel:?}");
        trace!("params: {params:?}");

        let mut url = gcal_api_url(&["calendars", calendar_id, "events", "watch"])?;
        append_query_pairs(&mut url, params);

        let send = GcalSend::post_json(auth, url, channel)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalEventsWatch {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalChannel>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("events watch established");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
