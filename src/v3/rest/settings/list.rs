//! List the settings of a user (`settings.list`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/settings/list>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use serde::Serialize;
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        query::to_query_pairs,
        rest::settings::GcalSettings,
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// Query parameters for listing user settings (`settings.list`).
///
/// The same set drives [`crate::v3::rest::settings::watch`].
#[derive(Debug, Clone, Default, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalSettingsListParams<'a> {
    /// The maximum number of settings to return per page.
    pub max_results: Option<u32>,
    /// The page token from a previous list response.
    pub page_token: Option<&'a str>,
    /// The sync token from the last page of a previous listing,
    /// restricting the response to what changed since then.
    pub sync_token: Option<&'a str>,
}

/// I/O-free coroutine listing the settings of a user
/// (`settings.list`).
pub struct GcalSettingsList {
    send: GcalSend<GcalSettings>,
}

impl GcalSettingsList {
    /// Builds the `settings.list` request from the given query
    /// parameters.
    pub fn new(
        auth: &HttpAuthBearer,
        params: &GcalSettingsListParams,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar settings listing");
        trace!("params: {params:?}");

        let mut url = Url::parse(GCAL_API_BASE)?.join("users/me/settings")?;
        url.query_pairs_mut().extend_pairs(to_query_pairs(params));

        let send = GcalSend::get(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalSettingsList {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalSettings>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("settings listed");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
