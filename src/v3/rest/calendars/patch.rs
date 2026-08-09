//! Patch a calendar's metadata (`calendars.patch`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/calendars/patch>

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

        let url = Url::parse(GCAL_API_BASE)?.join(&format!("calendars/{calendar_id}"))?;
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
