//! Get an access control rule (`acl.get`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/acl/get>

use alloc::format;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        rest::acl::GcalAclRule,
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// I/O-free coroutine getting an access control rule by id
/// (`acl.get`).
pub struct GcalAclRuleGet {
    send: GcalSend<GcalAclRule>,
}

impl GcalAclRuleGet {
    /// Builds the `acl.get` request for the given calendar and rule id.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        rule_id: &str,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar acl rule retrieval");
        trace!("calendar_id: {calendar_id:?}");
        trace!("rule_id: {rule_id:?}");

        let url =
            Url::parse(GCAL_API_BASE)?.join(&format!("calendars/{calendar_id}/acl/{rule_id}"))?;
        let send = GcalSend::get(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalAclRuleGet {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalAclRule>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("acl rule retrieved");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
