//! Replace an event (`events.update`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/events/update>

use alloc::format;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use serde::Serialize;
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        query::{append_query_pairs, is_false},
        rest::events::{GcalEvent, GcalSendUpdates},
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// Query parameters for replacing an event (`events.update`).
#[derive(Debug, Clone, Default, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalEventUpdateParams {
    /// Who gets notified of the change by email.
    pub send_updates: Option<GcalSendUpdates>,
    /// The version of the conference data the client supports, `0` for
    /// none and `1` for the create requests it can honour. Conference
    /// data on the event is ignored when it is left unset.
    pub conference_data_version: Option<u8>,
    /// The version of the event labels the client supports; the event
    /// label id is only honoured when it is `1`.
    pub event_label_version: Option<u8>,
    /// The maximum number of attendees to return on the updated event.
    pub max_attendees: Option<u32>,
    /// Whether the client supports attachments; they are dropped
    /// otherwise.
    #[serde(skip_serializing_if = "is_false")]
    pub supports_attachments: bool,
}

/// I/O-free coroutine replacing an event (`events.update`).
pub struct GcalEventUpdate {
    send: GcalSend<GcalEvent>,
}

impl GcalEventUpdate {
    /// Builds the `events.update` request, replacing the event of the
    /// given id as a whole: every field the event leaves unset is
    /// cleared.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        event_id: &str,
        event: &GcalEvent,
        params: &GcalEventUpdateParams,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar event for update");
        trace!("calendar_id: {calendar_id:?}");
        trace!("event_id: {event_id:?}");
        trace!("event: {event:?}");
        trace!("params: {params:?}");

        let mut url = Url::parse(GCAL_API_BASE)?
            .join(&format!("calendars/{calendar_id}/events/{event_id}"))?;
        append_query_pairs(&mut url, params);

        let send = GcalSend::put_json(auth, url, event)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalEventUpdate {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalEvent>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("event updated");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
