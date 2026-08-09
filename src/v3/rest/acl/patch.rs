//! Patch an access control rule (`acl.patch`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/acl/patch>

use alloc::{format, string::ToString};

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

/// I/O-free coroutine patching an access control rule (`acl.patch`).
pub struct GcalAclRulePatch {
    send: GcalSend<GcalAclRule>,
}

impl GcalAclRulePatch {
    /// Builds the `acl.patch` request from the given rule, merging only
    /// the fields it sets into the rule of the same id.
    ///
    /// `send_notifications` controls whether the grantee is notified by
    /// email; leaving it unset lets the API apply its default, which is
    /// to notify.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        rule_id: &str,
        rule: &GcalAclRule,
        send_notifications: Option<bool>,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar acl rule for patch");
        trace!("calendar_id: {calendar_id:?}");
        trace!("rule_id: {rule_id:?}");
        trace!("rule: {rule:?}");

        let mut url =
            Url::parse(GCAL_API_BASE)?.join(&format!("calendars/{calendar_id}/acl/{rule_id}"))?;

        if let Some(send_notifications) = send_notifications {
            url.query_pairs_mut()
                .append_pair("sendNotifications", &send_notifications.to_string());
        }

        let send = GcalSend::patch_json(auth, url, rule)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalAclRulePatch {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalAclRule>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("acl rule patched");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
