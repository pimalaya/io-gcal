//! Get a calendar (`calendars.get`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/calendars/get>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        rest::calendars::GcalCalendar,
        send::{GcalSend, GcalSendError, GcalSendOutput, gcal_api_url},
    },
};

/// I/O-free coroutine getting a calendar by id (`calendars.get`).
pub struct GcalCalendarGet {
    send: GcalSend<GcalCalendar>,
}

impl GcalCalendarGet {
    /// Builds the `calendars.get` request for the given calendar id.
    pub fn new(auth: &HttpAuthBearer, calendar_id: &str) -> Result<Self, GcalSendError> {
        debug!("prepare calendar retrieval");
        trace!("calendar_id: {calendar_id:?}");

        let url = gcal_api_url(&["calendars", calendar_id])?;
        let send = GcalSend::get(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalCalendarGet {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalCalendar>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("calendar retrieved");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
