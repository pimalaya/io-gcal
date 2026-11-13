//! Patch a calendar's metadata (`calendars.patch`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/calendars/patch>

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

/// I/O-free coroutine patching a calendar's metadata
/// (`calendars.patch`).
pub struct GcalCalendarPatch {
    send: GcalSend<GcalCalendar>,
}

impl GcalCalendarPatch {
    /// Builds the `calendars.patch` request, merging only the fields
    /// the given calendar sets.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        calendar: &GcalCalendar,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar for patch");
        trace!("calendar_id: {calendar_id:?}");
        trace!("calendar: {calendar:?}");

        let url = gcal_api_url(&["calendars", calendar_id])?;
        let send = GcalSend::patch_json(auth, url, calendar)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalCalendarPatch {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalCalendar>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("calendar patched");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
