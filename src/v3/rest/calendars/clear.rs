//! Empty a primary calendar (`calendars.clear`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/calendars/clear>

use alloc::format;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::send::{GCAL_API_BASE, GcalNoResponse, GcalSend, GcalSendError, GcalSendOutput},
};

/// I/O-free coroutine deleting every event of a primary calendar
/// (`calendars.clear`).
pub struct GcalCalendarClear {
    send: GcalSend<GcalNoResponse>,
}

impl GcalCalendarClear {
    /// Builds the `calendars.clear` request for the given calendar id.
    ///
    /// Only a primary calendar can be cleared; a secondary one is
    /// emptied by deleting it.
    pub fn new(auth: &HttpAuthBearer, calendar_id: &str) -> Result<Self, GcalSendError> {
        debug!("prepare calendar clear");
        trace!("calendar_id: {calendar_id:?}");

        let url = Url::parse(GCAL_API_BASE)?.join(&format!("calendars/{calendar_id}/clear"))?;
        let send = GcalSend::post_empty(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalCalendarClear {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalNoResponse>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("calendar cleared");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
