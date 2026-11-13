//! Get one entry of a user's calendar list (`calendarList.get`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/calendarList/get>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        rest::calendar_list::GcalCalendarListEntry,
        send::{GcalSend, GcalSendError, GcalSendOutput, gcal_api_url},
    },
};

/// I/O-free coroutine getting one calendar list entry by calendar id
/// (`calendarList.get`).
pub struct GcalCalendarListEntryGet {
    send: GcalSend<GcalCalendarListEntry>,
}

impl GcalCalendarListEntryGet {
    /// Builds the `calendarList.get` request for the given calendar id.
    pub fn new(auth: &HttpAuthBearer, calendar_id: &str) -> Result<Self, GcalSendError> {
        debug!("prepare calendar list entry retrieval");
        trace!("calendar_id: {calendar_id:?}");

        let url = gcal_api_url(&["users", "me", "calendarList", calendar_id])?;
        let send = GcalSend::get(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalCalendarListEntryGet {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalCalendarListEntry>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("calendar list entry retrieved");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
