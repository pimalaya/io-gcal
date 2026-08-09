//! Create an access control rule (`acl.insert`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/acl/insert>

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

/// I/O-free coroutine creating an access control rule (`acl.insert`).
pub struct GcalAclRuleInsert {
    send: GcalSend<GcalAclRule>,
}

impl GcalAclRuleInsert {
    /// Builds the `acl.insert` request from the given rule.
    ///
    /// `send_notifications` controls whether the grantee is notified by
    /// email; leaving it unset lets the API apply its default, which is
    /// to notify.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        rule: &GcalAclRule,
        send_notifications: Option<bool>,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar acl rule for creation");
        trace!("calendar_id: {calendar_id:?}");
        trace!("rule: {rule:?}");

        let mut url = Url::parse(GCAL_API_BASE)?.join(&format!("calendars/{calendar_id}/acl"))?;

        if let Some(send_notifications) = send_notifications {
            url.query_pairs_mut()
                .append_pair("sendNotifications", &send_notifications.to_string());
        }

        let send = GcalSend::post_json(auth, url, rule)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalAclRuleInsert {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalAclRule>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("acl rule created");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
