//! Transfer the data ownership of a calendar
//! (`calendars.transferOwnership`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/calendars/transferOwnership>

use alloc::{format, string::ToString};

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::send::{GCAL_API_BASE, GcalNoResponse, GcalSend, GcalSendError, GcalSendOutput},
};

/// I/O-free coroutine handing the data ownership of a calendar to
/// another user (`calendars.transferOwnership`).
pub struct GcalCalendarTransferOwnership {
    send: GcalSend<GcalNoResponse>,
}

impl GcalCalendarTransferOwnership {
    /// Builds the `calendars.transferOwnership` request for the given
    /// calendar id and new data owner.
    ///
    /// A calendar has a single data owner, distinct from the users
    /// holding the owner role on it. `use_admin_access` runs the
    /// transfer with domain administrator privileges, and is required
    /// by the API rather than optional.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        new_data_owner: &str,
        use_admin_access: bool,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar ownership transfer");
        trace!("calendar_id: {calendar_id:?}");
        trace!("new_data_owner: {new_data_owner:?}");
        trace!("use_admin_access: {use_admin_access:?}");

        let mut url = Url::parse(GCAL_API_BASE)?
            .join(&format!("calendars/{calendar_id}/transferOwnership"))?;

        url.query_pairs_mut()
            .append_pair("newDataOwner", new_data_owner)
            .append_pair("useAdminAccess", &use_admin_access.to_string());

        let send = GcalSend::post_empty(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalCalendarTransferOwnership {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalNoResponse>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("calendar ownership transferred");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
