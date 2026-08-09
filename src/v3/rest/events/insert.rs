//! Create an event (`events.insert`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/events/insert>

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
        rest::events::{GcalEvent, GcalSendUpdates},
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// Query parameters for creating an event (`events.insert`).
#[derive(Debug, Clone, Default, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalEventInsertParams {
    /// Who gets notified of the new event by email.
    pub send_updates: Option<GcalSendUpdates>,
    /// The version of the conference data the client supports, `0` for
    /// none and `1` for the create requests it can honour. Conference
    /// data on the event is ignored when it is left unset.
    pub conference_data_version: Option<u8>,
    /// The version of the event labels the client supports; the event
    /// label id is only honoured when it is `1`.
    pub event_label_version: Option<u8>,
    /// The maximum number of attendees to return on the created event.
    pub max_attendees: Option<u32>,
    /// Whether the client supports attachments; they are dropped
    /// otherwise.
    #[serde(skip_serializing_if = "is_false")]
    pub supports_attachments: bool,
}

/// I/O-free coroutine creating an event (`events.insert`).
pub struct GcalEventInsert {
    send: GcalSend<GcalEvent>,
}

impl GcalEventInsert {
    /// Builds the `events.insert` request from the given event, whose
    /// start and end are the only required fields.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        event: &GcalEvent,
        params: &GcalEventInsertParams,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar event for creation");
        trace!("calendar_id: {calendar_id:?}");
        trace!("event: {event:?}");
        trace!("params: {params:?}");

        if event.start.is_none() || event.end.is_none() {
            let err = GcalSendError::InvalidRequest("Event start and end are required".into());
            return Err(err);
        }

        let mut url =
            Url::parse(GCAL_API_BASE)?.join(&format!("calendars/{calendar_id}/events"))?;
        url.query_pairs_mut().extend_pairs(to_query_pairs(params));

        let send = GcalSend::post_json(auth, url, event)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalEventInsert {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalEvent>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("event created");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
