//! Create a secondary calendar (`calendars.insert`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/calendars/insert>

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

/// I/O-free coroutine creating a secondary calendar
/// (`calendars.insert`).
pub struct GcalCalendarInsert {
    send: GcalSend<GcalCalendar>,
}

impl GcalCalendarInsert {
    /// Builds the `calendars.insert` request from the given calendar,
    /// whose summary is the only required field.
    pub fn new(auth: &HttpAuthBearer, calendar: &GcalCalendar) -> Result<Self, GcalSendError> {
        debug!("prepare calendar for creation");
        trace!("calendar: {calendar:?}");

        let summary_is_empty = calendar
            .summary
            .as_ref()
            .is_none_or(|summary| summary.trim().is_empty());

        if summary_is_empty {
            let err = GcalSendError::InvalidRequest("Calendar summary cannot be empty".into());
            return Err(err);
        }

        let url = Url::parse(GCAL_API_BASE)?.join("calendars")?;
        let send = GcalSend::post_json(auth, url, calendar)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalCalendarInsert {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalCalendar>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("calendar created");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
