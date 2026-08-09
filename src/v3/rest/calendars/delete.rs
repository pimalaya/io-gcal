//! Delete a secondary calendar (`calendars.delete`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/calendars/delete>

use alloc::format;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::send::{GCAL_API_BASE, GcalNoResponse, GcalSend, GcalSendError, GcalSendOutput},
};

/// I/O-free coroutine deleting a secondary calendar
/// (`calendars.delete`).
pub struct GcalCalendarDelete {
    send: GcalSend<GcalNoResponse>,
}

impl GcalCalendarDelete {
    /// Builds the `calendars.delete` request for the given calendar id.
    ///
    /// Only secondary calendars can be deleted; use
    /// [`crate::v3::rest::calendars::clear`] to empty the primary one.
    pub fn new(auth: &HttpAuthBearer, calendar_id: &str) -> Result<Self, GcalSendError> {
        debug!("prepare calendar deletion");
        trace!("calendar_id: {calendar_id:?}");

        let url = Url::parse(GCAL_API_BASE)?.join(&format!("calendars/{calendar_id}"))?;
        let send = GcalSend::delete(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalCalendarDelete {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalNoResponse>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("calendar deleted");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
