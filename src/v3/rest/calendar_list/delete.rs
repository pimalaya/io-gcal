//! Unsubscribe a user from a calendar (`calendarList.delete`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/calendarList/delete>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};

use crate::{
    coroutine::*,
    gcal_try,
    v3::send::{GcalNoResponse, GcalSend, GcalSendError, GcalSendOutput, gcal_api_url},
};

/// I/O-free coroutine removing one calendar from a user's calendar list
/// (`calendarList.delete`).
pub struct GcalCalendarListEntryDelete {
    send: GcalSend<GcalNoResponse>,
}

impl GcalCalendarListEntryDelete {
    /// Builds the `calendarList.delete` request for the given calendar
    /// id.
    ///
    /// This only unsubscribes the user; the calendar itself and its
    /// events are left untouched.
    pub fn new(auth: &HttpAuthBearer, calendar_id: &str) -> Result<Self, GcalSendError> {
        debug!("prepare calendar list entry deletion");
        trace!("calendar_id: {calendar_id:?}");

        let url = gcal_api_url(&["users", "me", "calendarList", calendar_id])?;
        let send = GcalSend::delete(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalCalendarListEntryDelete {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalNoResponse>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("calendar list entry deleted");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
