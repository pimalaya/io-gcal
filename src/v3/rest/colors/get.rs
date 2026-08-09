//! Get the global colour palettes (`colors.get`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/colors/get>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        rest::colors::GcalColors,
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// I/O-free coroutine getting the event and calendar colour palettes
/// (`colors.get`).
pub struct GcalColorsGet {
    send: GcalSend<GcalColors>,
}

impl GcalColorsGet {
    /// Builds the `colors.get` request.
    pub fn new(auth: &HttpAuthBearer) -> Result<Self, GcalSendError> {
        debug!("prepare calendar colors retrieval");

        let url = Url::parse(GCAL_API_BASE)?.join("colors")?;
        let send = GcalSend::get(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalColorsGet {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalColors>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("colors retrieved");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
