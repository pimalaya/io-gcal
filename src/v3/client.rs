//! Std-blocking Google Calendar client, gated behind the `client`
//! feature.
//!
//! Wraps a `Read + Write` stream plus the bearer credential and runs
//! the coroutines against `www.googleapis.com`.

#[cfg(any(
    feature = "rustls-aws",
    feature = "rustls-ring",
    feature = "native-tls"
))]
use core::time::Duration;
use core::{any::Any, fmt};

#[cfg(any(
    feature = "rustls-aws",
    feature = "rustls-ring",
    feature = "native-tls"
))]
use alloc::string::String;
use alloc::{boxed::Box, string::ToString};
use std::io::{self, Read, Write};

use io_http::rfc6750::bearer::HttpAuthBearer;
#[cfg(any(
    feature = "rustls-aws",
    feature = "rustls-ring",
    feature = "native-tls"
))]
use pimalaya_stream::{
    stream::{Stream, TcpConnectOptions, TlsConnectOptions},
    tls::Tls,
};
use thiserror::Error;
#[cfg(any(
    feature = "rustls-aws",
    feature = "rustls-ring",
    feature = "native-tls"
))]
use url::Url;

#[cfg(any(
    feature = "rustls-aws",
    feature = "rustls-ring",
    feature = "native-tls"
))]
use crate::v3::send::GCAL_API_BASE;
use crate::{
    coroutine::*,
    v3::rest::{
        acl::{
            GcalAcl, GcalAclRule, delete::GcalAclRuleDelete, get::GcalAclRuleGet,
            insert::GcalAclRuleInsert, list::GcalAclList, list::GcalAclListParams,
            patch::GcalAclRulePatch, update::GcalAclRuleUpdate, watch::GcalAclWatch,
        },
        calendar_list::{
            GcalCalendarList, GcalCalendarListEntry, delete::GcalCalendarListEntryDelete,
            get::GcalCalendarListEntryGet, insert::GcalCalendarListEntryInsert,
            list::GcalCalendarListList, list::GcalCalendarListListParams,
            patch::GcalCalendarListEntryPatch, update::GcalCalendarListEntryUpdate,
            watch::GcalCalendarListWatch,
        },
        calendars::{
            GcalCalendar, clear::GcalCalendarClear, delete::GcalCalendarDelete,
            get::GcalCalendarGet, insert::GcalCalendarInsert, patch::GcalCalendarPatch,
            transfer_ownership::GcalCalendarTransferOwnership, update::GcalCalendarUpdate,
        },
        channels::{GcalChannel, stop::GcalChannelStop},
        colors::{GcalColors, get::GcalColorsGet},
        events::{
            GcalEvent, GcalEvents, GcalSendUpdates, delete::GcalEventDelete, get::GcalEventGet,
            import::GcalEventImport, import::GcalEventImportParams, insert::GcalEventInsert,
            insert::GcalEventInsertParams, instances::GcalEventInstances,
            instances::GcalEventInstancesParams, list::GcalEventsList, list::GcalEventsListParams,
            r#move::GcalEventMove, patch::GcalEventPatch, patch::GcalEventPatchParams,
            quick_add::GcalEventQuickAdd, update::GcalEventUpdate, update::GcalEventUpdateParams,
            watch::GcalEventsWatch,
        },
        freebusy::{GcalFreeBusyRequest, GcalFreeBusyResponse, query::GcalFreeBusyQuery},
        settings::{
            GcalSetting, GcalSettings, get::GcalSettingGet, list::GcalSettingsList,
            list::GcalSettingsListParams, watch::GcalSettingsWatch,
        },
    },
    v3::send::{GcalNoResponse, GcalSendError, GcalSendOutput},
};

/// Errors that can occur on the std client.
#[derive(Debug, Error)]
pub enum GcalClientStdError {
    /// The Calendar exchange itself failed.
    #[error(transparent)]
    Send(#[from] GcalSendError),
    /// Reading from or writing to the stream failed.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Opening the TCP/TLS connection failed.
    #[cfg(any(
        feature = "rustls-aws",
        feature = "rustls-ring",
        feature = "native-tls"
    ))]
    #[error(transparent)]
    Tls(#[from] anyhow::Error),
    /// The API base URL carries no host to connect to.
    #[cfg(any(
        feature = "rustls-aws",
        feature = "rustls-ring",
        feature = "native-tls"
    ))]
    #[error("Calendar URL `{0}` has no host")]
    UrlMissingHost(String),
    /// The API base URL scheme is neither http nor https.
    #[cfg(any(
        feature = "rustls-aws",
        feature = "rustls-ring",
        feature = "native-tls"
    ))]
    #[error("Calendar URL `{url}` has unsupported scheme `{scheme}` (expected `http` or `https`)")]
    UrlUnsupportedScheme {
        /// The offending URL.
        url: String,
        /// The unsupported scheme it carries.
        scheme: String,
    },
}

/// Optional settings for [`GcalClientStd::connect`]; the TLS backend
/// default is the only one there is.
#[derive(Debug, Default)]
pub struct GcalClientStdConnectOptions {
    /// TLS backend configuration.
    #[cfg(any(
        feature = "rustls-aws",
        feature = "rustls-ring",
        feature = "native-tls"
    ))]
    pub tls: Tls,
}

const READ_BUFFER_SIZE: usize = 16 * 1024;

/// Standard, blocking Google Calendar client.
///
/// Owns the stream and the bearer credential; each convenience method
/// builds the matching coroutine and runs it to completion. Every
/// calendar is addressed explicitly by id, `primary` being the alias of
/// the authenticated user's own calendar.
pub struct GcalClientStd {
    /// The underlying TCP or TLS stream.
    pub stream: Box<dyn GcalStream>,
    /// The OAuth 2.0 bearer credential added to every request.
    pub auth: HttpAuthBearer,
}

impl GcalClientStd {
    /// Builds a client over an already-connected stream.
    pub fn new<S: Read + Write + Send + 'static>(stream: S, token: impl ToString) -> Self {
        Self {
            stream: Box::new(stream),
            auth: HttpAuthBearer::new(token.to_string()),
        }
    }

    /// Opens a TCP/TLS connection to `www.googleapis.com` and builds
    /// the client around it.
    #[cfg(any(
        feature = "rustls-aws",
        feature = "rustls-ring",
        feature = "native-tls"
    ))]
    pub fn connect(
        token: impl ToString,
        options: GcalClientStdConnectOptions,
    ) -> Result<Self, GcalClientStdError> {
        let GcalClientStdConnectOptions { tls } = options;

        let url = Url::parse(GCAL_API_BASE).expect("Calendar API base URL is valid");
        let host = url
            .host_str()
            .ok_or_else(|| GcalClientStdError::UrlMissingHost(url.to_string()))?;

        let stream = match url.scheme() {
            "http" => {
                let port = url.port().unwrap_or(80);
                Stream::connect_tcp(host, port, TcpConnectOptions::default())?
            }
            "https" => {
                let port = url.port().unwrap_or(443);
                let opts = TlsConnectOptions {
                    tls: tls.clone(),
                    ..Default::default()
                };

                Stream::connect_tls(host, port, opts)?
            }
            scheme => {
                return Err(GcalClientStdError::UrlUnsupportedScheme {
                    url: url.to_string(),
                    scheme: scheme.to_string(),
                });
            }
        };

        stream.set_read_timeout(Some(Duration::from_secs(30)))?;

        Ok(Self {
            stream: Box::new(stream),
            auth: HttpAuthBearer::new(token.to_string()),
        })
    }

    /// Replaces the underlying stream, e.g. after reconnecting.
    pub fn set_stream<S: Read + Write + Send + 'static>(&mut self, stream: S) {
        self.stream = Box::new(stream);
    }

    /// Runs the given coroutine to completion against the stream,
    /// reading on `WantsRead` and writing on `WantsWrite`.
    pub fn run<C, T>(&mut self, mut coroutine: C) -> Result<GcalSendOutput<T>, GcalClientStdError>
    where
        C: GcalCoroutine<Yield = GcalYield, Return = Result<GcalSendOutput<T>, GcalSendError>>,
    {
        let mut buf = [0u8; READ_BUFFER_SIZE];
        let mut arg: Option<&[u8]> = None;

        loop {
            match coroutine.resume(arg.take()) {
                GcalCoroutineState::Complete(Ok(out)) => return Ok(out),
                GcalCoroutineState::Complete(Err(err)) => return Err(err.into()),
                GcalCoroutineState::Yielded(GcalYield::WantsRead) => {
                    let n = self.stream.read(&mut buf)?;
                    arg = Some(&buf[..n]);
                }
                GcalCoroutineState::Yielded(GcalYield::WantsWrite(bytes)) => {
                    self.stream.write_all(&bytes)?;
                    arg = None;
                }
            }
        }
    }

    /// Lists the calendars of the user's calendar list
    /// (`calendarList.list`).
    pub fn calendar_list_list(
        &mut self,
        params: &GcalCalendarListListParams,
    ) -> Result<GcalSendOutput<GcalCalendarList>, GcalClientStdError> {
        let coroutine = GcalCalendarListList::new(&self.auth, params)?;
        self.run(coroutine)
    }

    /// Gets one calendar list entry (`calendarList.get`).
    pub fn calendar_list_entry_get(
        &mut self,
        calendar_id: &str,
    ) -> Result<GcalSendOutput<GcalCalendarListEntry>, GcalClientStdError> {
        let coroutine = GcalCalendarListEntryGet::new(&self.auth, calendar_id)?;
        self.run(coroutine)
    }

    /// Subscribes the user to an existing calendar
    /// (`calendarList.insert`).
    pub fn calendar_list_entry_insert(
        &mut self,
        entry: &GcalCalendarListEntry,
        color_rgb_format: Option<bool>,
    ) -> Result<GcalSendOutput<GcalCalendarListEntry>, GcalClientStdError> {
        let coroutine = GcalCalendarListEntryInsert::new(&self.auth, entry, color_rgb_format)?;
        self.run(coroutine)
    }

    /// Replaces one calendar list entry (`calendarList.update`).
    pub fn calendar_list_entry_update(
        &mut self,
        calendar_id: &str,
        entry: &GcalCalendarListEntry,
        color_rgb_format: Option<bool>,
    ) -> Result<GcalSendOutput<GcalCalendarListEntry>, GcalClientStdError> {
        let coroutine =
            GcalCalendarListEntryUpdate::new(&self.auth, calendar_id, entry, color_rgb_format)?;
        self.run(coroutine)
    }

    /// Patches one calendar list entry (`calendarList.patch`).
    pub fn calendar_list_entry_patch(
        &mut self,
        calendar_id: &str,
        entry: &GcalCalendarListEntry,
        color_rgb_format: Option<bool>,
    ) -> Result<GcalSendOutput<GcalCalendarListEntry>, GcalClientStdError> {
        let coroutine =
            GcalCalendarListEntryPatch::new(&self.auth, calendar_id, entry, color_rgb_format)?;
        self.run(coroutine)
    }

    /// Unsubscribes the user from a calendar (`calendarList.delete`).
    pub fn calendar_list_entry_delete(
        &mut self,
        calendar_id: &str,
    ) -> Result<GcalSendOutput<GcalNoResponse>, GcalClientStdError> {
        let coroutine = GcalCalendarListEntryDelete::new(&self.auth, calendar_id)?;
        self.run(coroutine)
    }

    /// Watches the user's calendar list (`calendarList.watch`).
    pub fn calendar_list_watch(
        &mut self,
        channel: &GcalChannel,
        params: &GcalCalendarListListParams,
    ) -> Result<GcalSendOutput<GcalChannel>, GcalClientStdError> {
        let coroutine = GcalCalendarListWatch::new(&self.auth, channel, params)?;
        self.run(coroutine)
    }

    /// Gets a calendar by id (`calendars.get`).
    pub fn calendar_get(
        &mut self,
        calendar_id: &str,
    ) -> Result<GcalSendOutput<GcalCalendar>, GcalClientStdError> {
        let coroutine = GcalCalendarGet::new(&self.auth, calendar_id)?;
        self.run(coroutine)
    }

    /// Creates a secondary calendar (`calendars.insert`).
    pub fn calendar_insert(
        &mut self,
        calendar: &GcalCalendar,
    ) -> Result<GcalSendOutput<GcalCalendar>, GcalClientStdError> {
        let coroutine = GcalCalendarInsert::new(&self.auth, calendar)?;
        self.run(coroutine)
    }

    /// Replaces a calendar's metadata (`calendars.update`).
    pub fn calendar_update(
        &mut self,
        calendar_id: &str,
        calendar: &GcalCalendar,
    ) -> Result<GcalSendOutput<GcalCalendar>, GcalClientStdError> {
        let coroutine = GcalCalendarUpdate::new(&self.auth, calendar_id, calendar)?;
        self.run(coroutine)
    }

    /// Patches a calendar's metadata (`calendars.patch`).
    pub fn calendar_patch(
        &mut self,
        calendar_id: &str,
        calendar: &GcalCalendar,
    ) -> Result<GcalSendOutput<GcalCalendar>, GcalClientStdError> {
        let coroutine = GcalCalendarPatch::new(&self.auth, calendar_id, calendar)?;
        self.run(coroutine)
    }

    /// Deletes a secondary calendar (`calendars.delete`).
    pub fn calendar_delete(
        &mut self,
        calendar_id: &str,
    ) -> Result<GcalSendOutput<GcalNoResponse>, GcalClientStdError> {
        let coroutine = GcalCalendarDelete::new(&self.auth, calendar_id)?;
        self.run(coroutine)
    }

    /// Deletes every event of a primary calendar (`calendars.clear`).
    pub fn calendar_clear(
        &mut self,
        calendar_id: &str,
    ) -> Result<GcalSendOutput<GcalNoResponse>, GcalClientStdError> {
        let coroutine = GcalCalendarClear::new(&self.auth, calendar_id)?;
        self.run(coroutine)
    }

    /// Hands the data ownership of a calendar to another user
    /// (`calendars.transferOwnership`).
    pub fn calendar_transfer_ownership(
        &mut self,
        calendar_id: &str,
        new_data_owner: &str,
        use_admin_access: bool,
    ) -> Result<GcalSendOutput<GcalNoResponse>, GcalClientStdError> {
        let coroutine = GcalCalendarTransferOwnership::new(
            &self.auth,
            calendar_id,
            new_data_owner,
            use_admin_access,
        )?;
        self.run(coroutine)
    }

    /// Lists the events of a calendar (`events.list`).
    pub fn events_list(
        &mut self,
        calendar_id: &str,
        params: &GcalEventsListParams,
    ) -> Result<GcalSendOutput<GcalEvents>, GcalClientStdError> {
        let coroutine = GcalEventsList::new(&self.auth, calendar_id, params)?;
        self.run(coroutine)
    }

    /// Gets an event by id (`events.get`).
    pub fn event_get(
        &mut self,
        calendar_id: &str,
        event_id: &str,
        max_attendees: Option<u32>,
        time_zone: Option<&str>,
    ) -> Result<GcalSendOutput<GcalEvent>, GcalClientStdError> {
        let coroutine =
            GcalEventGet::new(&self.auth, calendar_id, event_id, max_attendees, time_zone)?;
        self.run(coroutine)
    }

    /// Creates an event (`events.insert`).
    pub fn event_insert(
        &mut self,
        calendar_id: &str,
        event: &GcalEvent,
        params: &GcalEventInsertParams,
    ) -> Result<GcalSendOutput<GcalEvent>, GcalClientStdError> {
        let coroutine = GcalEventInsert::new(&self.auth, calendar_id, event, params)?;
        self.run(coroutine)
    }

    /// Replaces an event (`events.update`), optionally guarded by the
    /// etag a read returned.
    pub fn event_update(
        &mut self,
        calendar_id: &str,
        event_id: &str,
        event: &GcalEvent,
        params: &GcalEventUpdateParams,
        if_match: Option<&str>,
    ) -> Result<GcalSendOutput<GcalEvent>, GcalClientStdError> {
        let coroutine =
            GcalEventUpdate::new(&self.auth, calendar_id, event_id, event, params, if_match)?;
        self.run(coroutine)
    }

    /// Patches an event (`events.patch`), optionally guarded by the
    /// etag a read returned.
    pub fn event_patch(
        &mut self,
        calendar_id: &str,
        event_id: &str,
        event: &GcalEvent,
        params: &GcalEventPatchParams,
        if_match: Option<&str>,
    ) -> Result<GcalSendOutput<GcalEvent>, GcalClientStdError> {
        let coroutine =
            GcalEventPatch::new(&self.auth, calendar_id, event_id, event, params, if_match)?;
        self.run(coroutine)
    }

    /// Deletes an event by id (`events.delete`), optionally guarded by
    /// the etag a read returned.
    pub fn event_delete(
        &mut self,
        calendar_id: &str,
        event_id: &str,
        send_updates: Option<GcalSendUpdates>,
        if_match: Option<&str>,
    ) -> Result<GcalSendOutput<GcalNoResponse>, GcalClientStdError> {
        let coroutine =
            GcalEventDelete::new(&self.auth, calendar_id, event_id, send_updates, if_match)?;
        self.run(coroutine)
    }

    /// Imports an existing event into a calendar (`events.import`).
    pub fn event_import(
        &mut self,
        calendar_id: &str,
        event: &GcalEvent,
        params: &GcalEventImportParams,
    ) -> Result<GcalSendOutput<GcalEvent>, GcalClientStdError> {
        let coroutine = GcalEventImport::new(&self.auth, calendar_id, event, params)?;
        self.run(coroutine)
    }

    /// Lists the instances of a recurring event (`events.instances`).
    pub fn event_instances(
        &mut self,
        calendar_id: &str,
        event_id: &str,
        params: &GcalEventInstancesParams,
    ) -> Result<GcalSendOutput<GcalEvents>, GcalClientStdError> {
        let coroutine = GcalEventInstances::new(&self.auth, calendar_id, event_id, params)?;
        self.run(coroutine)
    }

    /// Moves an event to another calendar (`events.move`).
    pub fn event_move(
        &mut self,
        calendar_id: &str,
        event_id: &str,
        destination: &str,
        send_updates: Option<GcalSendUpdates>,
    ) -> Result<GcalSendOutput<GcalEvent>, GcalClientStdError> {
        let coroutine =
            GcalEventMove::new(&self.auth, calendar_id, event_id, destination, send_updates)?;
        self.run(coroutine)
    }

    /// Creates an event from a piece of text (`events.quickAdd`).
    pub fn event_quick_add(
        &mut self,
        calendar_id: &str,
        text: &str,
        send_updates: Option<GcalSendUpdates>,
    ) -> Result<GcalSendOutput<GcalEvent>, GcalClientStdError> {
        let coroutine = GcalEventQuickAdd::new(&self.auth, calendar_id, text, send_updates)?;
        self.run(coroutine)
    }

    /// Watches the events of a calendar (`events.watch`).
    pub fn events_watch(
        &mut self,
        calendar_id: &str,
        channel: &GcalChannel,
        params: &GcalEventsListParams,
    ) -> Result<GcalSendOutput<GcalChannel>, GcalClientStdError> {
        let coroutine = GcalEventsWatch::new(&self.auth, calendar_id, channel, params)?;
        self.run(coroutine)
    }

    /// Lists the access control rules of a calendar (`acl.list`).
    pub fn acl_list(
        &mut self,
        calendar_id: &str,
        params: &GcalAclListParams,
    ) -> Result<GcalSendOutput<GcalAcl>, GcalClientStdError> {
        let coroutine = GcalAclList::new(&self.auth, calendar_id, params)?;
        self.run(coroutine)
    }

    /// Gets an access control rule by id (`acl.get`).
    pub fn acl_rule_get(
        &mut self,
        calendar_id: &str,
        rule_id: &str,
    ) -> Result<GcalSendOutput<GcalAclRule>, GcalClientStdError> {
        let coroutine = GcalAclRuleGet::new(&self.auth, calendar_id, rule_id)?;
        self.run(coroutine)
    }

    /// Creates an access control rule (`acl.insert`).
    pub fn acl_rule_insert(
        &mut self,
        calendar_id: &str,
        rule: &GcalAclRule,
        send_notifications: Option<bool>,
    ) -> Result<GcalSendOutput<GcalAclRule>, GcalClientStdError> {
        let coroutine = GcalAclRuleInsert::new(&self.auth, calendar_id, rule, send_notifications)?;
        self.run(coroutine)
    }

    /// Replaces an access control rule (`acl.update`).
    pub fn acl_rule_update(
        &mut self,
        calendar_id: &str,
        rule_id: &str,
        rule: &GcalAclRule,
        send_notifications: Option<bool>,
    ) -> Result<GcalSendOutput<GcalAclRule>, GcalClientStdError> {
        let coroutine =
            GcalAclRuleUpdate::new(&self.auth, calendar_id, rule_id, rule, send_notifications)?;
        self.run(coroutine)
    }

    /// Patches an access control rule (`acl.patch`).
    pub fn acl_rule_patch(
        &mut self,
        calendar_id: &str,
        rule_id: &str,
        rule: &GcalAclRule,
        send_notifications: Option<bool>,
    ) -> Result<GcalSendOutput<GcalAclRule>, GcalClientStdError> {
        let coroutine =
            GcalAclRulePatch::new(&self.auth, calendar_id, rule_id, rule, send_notifications)?;
        self.run(coroutine)
    }

    /// Deletes an access control rule by id (`acl.delete`).
    pub fn acl_rule_delete(
        &mut self,
        calendar_id: &str,
        rule_id: &str,
    ) -> Result<GcalSendOutput<GcalNoResponse>, GcalClientStdError> {
        let coroutine = GcalAclRuleDelete::new(&self.auth, calendar_id, rule_id)?;
        self.run(coroutine)
    }

    /// Watches the access control rules of a calendar (`acl.watch`).
    pub fn acl_watch(
        &mut self,
        calendar_id: &str,
        channel: &GcalChannel,
        params: &GcalAclListParams,
    ) -> Result<GcalSendOutput<GcalChannel>, GcalClientStdError> {
        let coroutine = GcalAclWatch::new(&self.auth, calendar_id, channel, params)?;
        self.run(coroutine)
    }

    /// Lists the settings of the user (`settings.list`).
    pub fn settings_list(
        &mut self,
        params: &GcalSettingsListParams,
    ) -> Result<GcalSendOutput<GcalSettings>, GcalClientStdError> {
        let coroutine = GcalSettingsList::new(&self.auth, params)?;
        self.run(coroutine)
    }

    /// Gets one setting by id (`settings.get`).
    pub fn setting_get(
        &mut self,
        setting: &str,
    ) -> Result<GcalSendOutput<GcalSetting>, GcalClientStdError> {
        let coroutine = GcalSettingGet::new(&self.auth, setting)?;
        self.run(coroutine)
    }

    /// Watches the settings of the user (`settings.watch`).
    pub fn settings_watch(
        &mut self,
        channel: &GcalChannel,
        params: &GcalSettingsListParams,
    ) -> Result<GcalSendOutput<GcalChannel>, GcalClientStdError> {
        let coroutine = GcalSettingsWatch::new(&self.auth, channel, params)?;
        self.run(coroutine)
    }

    /// Gets the event and calendar colour palettes (`colors.get`).
    pub fn colors_get(&mut self) -> Result<GcalSendOutput<GcalColors>, GcalClientStdError> {
        let coroutine = GcalColorsGet::new(&self.auth)?;
        self.run(coroutine)
    }

    /// Queries the free/busy information of a set of calendars
    /// (`freebusy.query`).
    pub fn free_busy_query(
        &mut self,
        request: &GcalFreeBusyRequest,
    ) -> Result<GcalSendOutput<GcalFreeBusyResponse>, GcalClientStdError> {
        let coroutine = GcalFreeBusyQuery::new(&self.auth, request)?;
        self.run(coroutine)
    }

    /// Closes a notification channel (`channels.stop`).
    pub fn channel_stop(
        &mut self,
        channel: &GcalChannel,
    ) -> Result<GcalSendOutput<GcalNoResponse>, GcalClientStdError> {
        let coroutine = GcalChannelStop::new(&self.auth, channel)?;
        self.run(coroutine)
    }
}

impl fmt::Debug for GcalClientStd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GcalClientStd")
            .field("auth", &self.auth)
            .finish_non_exhaustive()
    }
}

/// Boxable client stream: `Read + Write + Send` plus `Any` so callers
/// can downcast back to the concrete stream type.
pub trait GcalStream: Read + Write + Send + Any {
    /// Returns the stream as a mutable `Any` for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Read + Write + Send + Any> GcalStream for T {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
