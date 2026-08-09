//! Watch a user's calendar list for changes (`calendarList.watch`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/calendarList/watch>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        query::to_query_pairs,
        rest::{calendar_list::list::GcalCalendarListListParams, channels::GcalChannel},
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// I/O-free coroutine opening a notification channel on a user's
/// calendar list (`calendarList.watch`).
pub struct GcalCalendarListWatch {
    send: GcalSend<GcalChannel>,
}

impl GcalCalendarListWatch {
    /// Builds the `calendarList.watch` request from the given channel,
    /// which must carry at least its id, its type and its address.
    ///
    /// The params narrow the watched set exactly as they narrow a
    /// listing.
    pub fn new(
        auth: &HttpAuthBearer,
        channel: &GcalChannel,
        params: &GcalCalendarListListParams,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar list watch");
        trace!("channel: {channel:?}");
        trace!("params: {params:?}");

        let mut url = Url::parse(GCAL_API_BASE)?.join("users/me/calendarList/watch")?;
        url.query_pairs_mut().extend_pairs(to_query_pairs(params));

        let send = GcalSend::post_json(auth, url, channel)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalCalendarListWatch {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalChannel>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("calendar list watch established");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
