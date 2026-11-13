//! Watch the access control rules of a calendar for changes
//! (`acl.watch`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/acl/watch>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        query::append_query_pairs,
        rest::{acl::list::GcalAclListParams, channels::GcalChannel},
        send::{GcalSend, GcalSendError, GcalSendOutput, gcal_api_url},
    },
};

/// I/O-free coroutine opening a notification channel on the access
/// control rules of a calendar (`acl.watch`).
pub struct GcalAclWatch {
    send: GcalSend<GcalChannel>,
}

impl GcalAclWatch {
    /// Builds the `acl.watch` request from the given channel, which
    /// must carry at least its id, its type and its address.
    ///
    /// The params narrow the watched set exactly as they narrow a
    /// listing.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        channel: &GcalChannel,
        params: &GcalAclListParams,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar acl watch");
        trace!("calendar_id: {calendar_id:?}");
        trace!("channel: {channel:?}");
        trace!("params: {params:?}");

        let mut url = gcal_api_url(&["calendars", calendar_id, "acl", "watch"])?;
        append_query_pairs(&mut url, params);

        let send = GcalSend::post_json(auth, url, channel)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalAclWatch {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalChannel>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("acl watch established");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
