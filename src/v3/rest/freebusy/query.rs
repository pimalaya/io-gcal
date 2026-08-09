//! Query the free/busy information of a set of calendars
//! (`freebusy.query`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/freebusy/query>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        rest::freebusy::{GcalFreeBusyRequest, GcalFreeBusyResponse},
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// I/O-free coroutine querying the free/busy information of a set of
/// calendars (`freebusy.query`).
pub struct GcalFreeBusyQuery {
    send: GcalSend<GcalFreeBusyResponse>,
}

impl GcalFreeBusyQuery {
    /// Builds the `freebusy.query` request from the given query, whose
    /// interval bounds and calendar list are all required.
    pub fn new(
        auth: &HttpAuthBearer,
        request: &GcalFreeBusyRequest,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar free/busy query");
        trace!("request: {request:?}");

        if request.items.is_empty() {
            let err =
                GcalSendError::InvalidRequest("Free/busy query needs at least one calendar".into());
            return Err(err);
        }

        let url = Url::parse(GCAL_API_BASE)?.join("freeBusy")?;
        let send = GcalSend::post_json(auth, url, request)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalFreeBusyQuery {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalFreeBusyResponse>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("free/busy queried");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
