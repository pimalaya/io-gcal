//! Get one setting of a user (`settings.get`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/settings/get>

use alloc::format;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        rest::settings::GcalSetting,
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// I/O-free coroutine getting one user setting by id
/// (`settings.get`).
pub struct GcalSettingGet {
    send: GcalSend<GcalSetting>,
}

impl GcalSettingGet {
    /// Builds the `settings.get` request for the given setting id, such
    /// as `timezone` or `weekStart`.
    pub fn new(auth: &HttpAuthBearer, setting: &str) -> Result<Self, GcalSendError> {
        debug!("prepare calendar setting retrieval");
        trace!("setting: {setting:?}");

        let url = Url::parse(GCAL_API_BASE)?.join(&format!("users/me/settings/{setting}"))?;
        let send = GcalSend::get(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalSettingGet {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalSetting>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("setting retrieved");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
