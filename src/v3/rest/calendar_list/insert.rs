//! Subscribe a user to a calendar (`calendarList.insert`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/calendarList/insert>

use alloc::string::ToString;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        rest::calendar_list::GcalCalendarListEntry,
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// I/O-free coroutine adding an existing calendar to a user's calendar
/// list (`calendarList.insert`).
pub struct GcalCalendarListEntryInsert {
    send: GcalSend<GcalCalendarListEntry>,
}

impl GcalCalendarListEntryInsert {
    /// Builds the `calendarList.insert` request from the given entry,
    /// whose id is the only required field.
    ///
    /// `color_rgb_format` must be set for the entry's explicit
    /// foreground and background colours to be taken into account,
    /// rather than its index-based colour id.
    pub fn new(
        auth: &HttpAuthBearer,
        entry: &GcalCalendarListEntry,
        color_rgb_format: Option<bool>,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar list entry for creation");
        trace!("entry: {entry:?}");

        let id_is_empty = entry.id.as_ref().is_none_or(|id| id.trim().is_empty());

        if id_is_empty {
            let err = GcalSendError::InvalidRequest("Calendar id cannot be empty".into());
            return Err(err);
        }

        let mut url = Url::parse(GCAL_API_BASE)?.join("users/me/calendarList")?;

        if let Some(color_rgb_format) = color_rgb_format {
            url.query_pairs_mut()
                .append_pair("colorRgbFormat", &color_rgb_format.to_string());
        }

        let send = GcalSend::post_json(auth, url, entry)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalCalendarListEntryInsert {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalCalendarListEntry>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("calendar list entry created");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
