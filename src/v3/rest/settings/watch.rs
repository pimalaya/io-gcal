//! Watch the settings of a user for changes (`settings.watch`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/settings/watch>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        query::append_query_pairs,
        rest::{channels::GcalChannel, settings::list::GcalSettingsListParams},
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// I/O-free coroutine opening a notification channel on the settings of
/// a user (`settings.watch`).
pub struct GcalSettingsWatch {
    send: GcalSend<GcalChannel>,
}

impl GcalSettingsWatch {
    /// Builds the `settings.watch` request from the given channel,
    /// which must carry at least its id, its type and its address.
    ///
    /// The params narrow the watched set exactly as they narrow a
    /// listing.
    pub fn new(
        auth: &HttpAuthBearer,
        channel: &GcalChannel,
        params: &GcalSettingsListParams,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar settings watch");
        trace!("channel: {channel:?}");
        trace!("params: {params:?}");

        let mut url = Url::parse(GCAL_API_BASE)?.join("users/me/settings/watch")?;
        append_query_pairs(&mut url, params);

        let send = GcalSend::post_json(auth, url, channel)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalSettingsWatch {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalChannel>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("settings watch established");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
