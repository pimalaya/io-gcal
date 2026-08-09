//! List the calendars of a user's calendar list
//! (`calendarList.list`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/calendarList/list>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use serde::Serialize;
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        query::{is_false, to_query_pairs},
        rest::{acl::GcalAccessRole, calendar_list::GcalCalendarList},
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// Query parameters for listing the calendar list
/// (`calendarList.list`).
///
/// The same set drives [`crate::v3::rest::calendar_list::watch`].
#[derive(Debug, Clone, Default, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalCalendarListListParams<'a> {
    /// The maximum number of entries to return per page.
    pub max_results: Option<u32>,
    /// The minimum access role the user must have on the returned
    /// calendars. The API rejects [`GcalAccessRole::None`] here.
    pub min_access_role: Option<GcalAccessRole>,
    /// The page token from a previous list response.
    pub page_token: Option<&'a str>,
    /// Whether to include the entries deleted from the calendar list.
    #[serde(skip_serializing_if = "is_false")]
    pub show_deleted: bool,
    /// Whether to include the entries hidden from the calendar list.
    #[serde(skip_serializing_if = "is_false")]
    pub show_hidden: bool,
    /// Whether to restrict the response to the calendars of the user's
    /// own organization.
    #[serde(skip_serializing_if = "is_false")]
    pub show_own_organization_only: bool,
    /// The sync token from the last page of a previous listing,
    /// restricting the response to what changed since then.
    pub sync_token: Option<&'a str>,
}

/// I/O-free coroutine listing the calendars of a user's calendar list
/// (`calendarList.list`).
pub struct GcalCalendarListList {
    send: GcalSend<GcalCalendarList>,
}

impl GcalCalendarListList {
    /// Builds the `calendarList.list` request from the given query
    /// parameters.
    pub fn new(
        auth: &HttpAuthBearer,
        params: &GcalCalendarListListParams,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar list listing");
        trace!("params: {params:?}");

        let mut url = Url::parse(GCAL_API_BASE)?.join("users/me/calendarList")?;
        url.query_pairs_mut().extend_pairs(to_query_pairs(params));

        let send = GcalSend::get(auth, url);

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalCalendarListList {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalCalendarList>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("calendar list listed");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
