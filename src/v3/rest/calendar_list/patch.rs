//! Patch one entry of a user's calendar list (`calendarList.patch`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/calendarList/patch>

use alloc::{format, string::ToString};

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

/// I/O-free coroutine patching one calendar list entry
/// (`calendarList.patch`).
pub struct GcalCalendarListEntryPatch {
    send: GcalSend<GcalCalendarListEntry>,
}

impl GcalCalendarListEntryPatch {
    /// Builds the `calendarList.patch` request, merging only the fields
    /// the given entry sets into the entry of the given calendar id.
    ///
    /// `color_rgb_format` must be set for the entry's explicit
    /// foreground and background colours to be taken into account,
    /// rather than its index-based colour id.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        entry: &GcalCalendarListEntry,
        color_rgb_format: Option<bool>,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar list entry for patch");
        trace!("calendar_id: {calendar_id:?}");
        trace!("entry: {entry:?}");

        let mut url =
            Url::parse(GCAL_API_BASE)?.join(&format!("users/me/calendarList/{calendar_id}"))?;

        if let Some(color_rgb_format) = color_rgb_format {
            url.query_pairs_mut()
                .append_pair("colorRgbFormat", &color_rgb_format.to_string());
        }

        let send = GcalSend::patch_json(auth, url, entry)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalCalendarListEntryPatch {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalCalendarListEntry>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("calendar list entry patched");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
