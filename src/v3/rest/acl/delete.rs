//! Delete an access control rule (`acl.delete`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/acl/delete>

use alloc::format;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::send::{GCAL_API_BASE, GcalNoResponse, GcalSend, GcalSendError, GcalSendOutput},
};

/// I/O-free coroutine deleting an access control rule by id
/// (`acl.delete`).
pub struct GcalAclRuleDelete {
    send: GcalSend<GcalNoResponse>,
}

impl GcalAclRuleDelete {
    /// Builds the `acl.delete` request for the given calendar and rule
    /// id.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        rule_id: &str,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar acl rule deletion");
        trace!("calendar_id: {calendar_id:?}");
        trace!("rule_id: {rule_id:?}");

        let url =
            Url::parse(GCAL_API_BASE)?.join(&format!("calendars/{calendar_id}/acl/{rule_id}"))?;
        let send = GcalSend::delete(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalAclRuleDelete {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalNoResponse>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("acl rule deleted");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
