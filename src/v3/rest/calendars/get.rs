//! Get a calendar (`calendars.get`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/calendars/get>

use alloc::format;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        rest::calendars::GcalCalendar,
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
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

        let url = Url::parse(GCAL_API_BASE)?.join(&format!("calendars/{calendar_id}"))?;
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
