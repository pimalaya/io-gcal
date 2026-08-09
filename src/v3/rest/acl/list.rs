//! List the access control rules of a calendar (`acl.list`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/acl/list>

use alloc::format;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use serde::Serialize;
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        query::{is_false, to_query_pairs},
        rest::acl::GcalAcl,
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// Query parameters for listing access control rules (`acl.list`).
///
/// The same set drives [`crate::v3::rest::acl::watch`].
#[derive(Debug, Clone, Default, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalAclListParams<'a> {
    /// The maximum number of rules to return per page.
    pub max_results: Option<u32>,
    /// The page token from a previous list response.
    pub page_token: Option<&'a str>,
    /// Whether to include deleted rules, with their role set to
    /// [`crate::v3::rest::acl::GcalAccessRole::None`].
    #[serde(skip_serializing_if = "is_false")]
    pub show_deleted: bool,
    /// The sync token from the last page of a previous listing,
    /// restricting the response to what changed since then.
    pub sync_token: Option<&'a str>,
}

/// I/O-free coroutine listing the access control rules of a calendar
/// (`acl.list`).
pub struct GcalAclList {
    send: GcalSend<GcalAcl>,
}

impl GcalAclList {
    /// Builds the `acl.list` request for the given calendar.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        params: &GcalAclListParams,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar acl listing");
        trace!("calendar_id: {calendar_id:?}");
        trace!("params: {params:?}");

        let mut url = Url::parse(GCAL_API_BASE)?.join(&format!("calendars/{calendar_id}/acl"))?;
        url.query_pairs_mut().extend_pairs(to_query_pairs(params));

        let send = GcalSend::get(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalAclList {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalAcl>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("acl listed");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
