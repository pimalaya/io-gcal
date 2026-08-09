//! Stop watching a resource through a channel (`channels.stop`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/channels/stop>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        rest::channels::GcalChannel,
        send::{GCAL_API_BASE, GcalNoResponse, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// I/O-free coroutine closing a notification channel (`channels.stop`).
pub struct GcalChannelStop {
    send: GcalSend<GcalNoResponse>,
}

impl GcalChannelStop {
    /// Builds the `channels.stop` request for the given channel, which
    /// must carry at least its id and resource id.
    pub fn new(auth: &HttpAuthBearer, channel: &GcalChannel) -> Result<Self, GcalSendError> {
        debug!("prepare calendar channel stop");
        trace!("channel: {channel:?}");

        let url = Url::parse(GCAL_API_BASE)?.join("channels/stop")?;
        let send = GcalSend::post_json(auth, url, channel)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalChannelStop {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalNoResponse>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("channel stopped");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
