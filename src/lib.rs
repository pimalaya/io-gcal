#![no_std]
#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! # io-gcal
//!
//! I/O-free coroutines for the [Google Calendar REST API], built on
//! [io-http] (HTTP/1.1) and pumped by any stream the caller owns.
//!
//! [Google Calendar REST API]: https://developers.google.com/workspace/calendar/api/v3/reference
//! [io-http]: https://docs.rs/io-http
//!
//! io-gcal is the calendar sibling of [io-gmail] (mail) and
//! [io-gpeople] (contacts): same shape, same vendor, a third Google API.
//! The crate name carries the vendor because "calendar" alone says
//! nothing about who serves it, the way "msgraph" carries it for
//! Microsoft.
//!
//! [io-gmail]: https://docs.rs/io-gmail
//! [io-gpeople]: https://docs.rs/io-gpeople
//!
//! ## Layers and features
//!
//! The crate has two of the three standard Pimalaya layers; there is no
//! CLI:
//!
//! 1. **I/O-free coroutines** (`no_std` core, always present): the whole
//!    Calendar REST logic.
//! 2. **Std client** ([`v3::client::GcalClientStd`], `client` feature):
//!    a blocking pump over any stream, with `connect` opening the
//!    TCP/TLS connection itself behind a TLS feature (`rustls-ring` by
//!    default, `rustls-aws`, `native-tls`).
//!
//! ## Everything lives under v3
//!
//! The Calendar REST API is versioned (`/calendar/v3/`), so the crate
//! is too: the version-agnostic [`coroutine`] contract stays at the
//! crate root, everything else lives under [`v3`]. The day Calendar
//! ships a v4, a sibling module slots in without breaking `v3`
//! consumers.
//!
//! ## The coroutine contract
//!
//! Every exchange implements [`coroutine::GcalCoroutine`]: `resume`
//! takes the bytes read since the last yield and either requests I/O
//! ([`coroutine::GcalYield`] `WantsRead` / `WantsWrite`) or completes.
//! The [`gcal_try!`] macro is the coroutine equivalent of `?`.
//!
//! A Calendar call is a single HTTP request/response, so every REST
//! coroutine is a thin wrapper around one shared primitive,
//! [`v3::send::GcalSend`]: it builds the authorized request (bearer
//! token, JSON in and out) and parses either the 2xx body or Calendar's
//! error envelope into [`v3::send::GcalSendError`]. Redirects are never
//! followed. The terminal [`v3::send::GcalSendOutput`] carries the
//! parsed response plus a keep-alive flag so pumps can reuse the
//! connection across the many small requests a sync makes. Empty 2xx
//! bodies (delete, clear, transfer ownership, channel stop)
//! deserialize into the [`v3::send::GcalNoResponse`] unit marker.
//!
//! ## Naming
//!
//! Public items follow `<Domain><Target><Verb><Ext>`: the domain is
//! `Gcal`, the target-verb pair mirrors the REST method
//! ([`v3::rest::events::get::GcalEventGet`] for `events.get`,
//! [`v3::rest::calendars::transfer_ownership::GcalCalendarTransferOwnership`]
//! for `calendars.transferOwnership`) and the extension distinguishes
//! companions (`Params`, `Error`, `Yield`).
//!
//! A plural target marks a collection-level method and a singular one
//! an item-level method, which is what tells
//! [`v3::rest::events::list::GcalEventsList`] (the events of a
//! calendar) from [`v3::rest::events::get::GcalEventGet`] (one of
//! them). Pure data resources omit the verb and keep the name the
//! reference gives them, collections included:
//! [`v3::rest::events::GcalEvent`] and [`v3::rest::events::GcalEvents`],
//! [`v3::rest::acl::GcalAclRule`] and [`v3::rest::acl::GcalAcl`]. Where
//! Google's own resource name already ends in a noun that reads like a
//! verb, the stutter is inherited rather than invented:
//! `calendarList.list` becomes
//! [`v3::rest::calendar_list::list::GcalCalendarListList`].
//!
//! ## Module layout
//!
//! [`v3::rest`] mirrors the Calendar REST reference one-to-one: each
//! resource is a directory and each method a file named after the API
//! method in snake_case (`quickAdd` becomes quick_add.rs). A reader who
//! knows the reference knows where to look.
//!
//! Request bodies take the whole resource by reference (`&GcalEvent`,
//! `&GcalCalendar`), so a `Default` resource with a few fields set
//! serializes cleanly. Enum-valued wire strings are typed enums; the
//! labels the reference leaves open, such as the features of a
//! conference entry point, stay strings. List methods take borrowed
//! `*Params` structs flattened into query pairs by
//! [`v3::query::to_query_pairs`], a tiny `no_std` serde serializer that
//! emits the repeated-key sequences Calendar expects; the methods
//! carrying one or two scalar parameters take them positionally
//! instead.
//!
//! ## Watching and incremental sync
//!
//! Calendar has both a push channel and sync tokens, and the two work
//! together. A `watch` method ([`v3::rest::events::watch`],
//! [`v3::rest::acl::watch`], [`v3::rest::calendar_list::watch`],
//! [`v3::rest::settings::watch`]) opens a
//! [`v3::rest::channels::GcalChannel`] delivering a content-free
//! notification to a webhook whenever the watched collection changes,
//! and [`v3::rest::channels::stop`] closes it. The notification says
//! only that something moved: the receiver answers it by replaying the
//! listing with the sync token of its last page, which returns just
//! what changed (deletions come back with a cancelled status or a
//! deleted flag). An expired token surfaces as HTTP 410, which
//! [`v3::send::GcalSendError::is_sync_token_expired`] recognises and
//! callers recover from by re-baselining a full listing.
//!
//! Unlike io-gmail, io-gcal ships no composite watcher coroutine: with
//! a real push channel there is no timer to own, so the loop belongs to
//! the caller that receives the webhook.
//!
//! ## Authentication
//!
//! io-gcal does no OAuth itself: the API only accepts OAuth 2.0 bearer
//! tokens, so the credential is exactly a bare access token, and
//! minting or refreshing it is the caller's responsibility.
//!
//! ## Intentional omissions
//!
//! The deprecated query parameters are left out: `alwaysIncludeEmail`,
//! which the reference documents as ignored, and the `sendNotifications`
//! of the events methods, superseded by `sendUpdates`. The
//! `sendNotifications` of the ACL methods is not deprecated and is
//! covered. Deprecated *fields* are kept, since the API still returns
//! them.
//!
//! ## Logging
//!
//! Coroutines pair a `debug!` lifecycle line with one `trace!` per
//! input variable in `new()`, and a `debug!` plus `trace!("out: ...")`
//! when `resume` completes; the crate never logs above `debug!`.
//!
//! ## Example
//!
//! Running a coroutine against a caller-owned TLS stream:
//!
//! ```rust,no_run
//! use std::{
//!     io::{Read, Write},
//!     net::TcpStream,
//!     sync::Arc,
//! };
//!
//! use io_gcal::{coroutine::*, v3::rest::events::list::{GcalEventsList, GcalEventsListParams}};
//! use io_http::rfc6750::bearer::HttpAuthBearer;
//! use rustls::{ClientConfig, ClientConnection, StreamOwned};
//! use rustls_platform_verifier::ConfigVerifierExt;
//!
//! let config = ClientConfig::with_platform_verifier().unwrap();
//! let server_name = "www.googleapis.com".try_into().unwrap();
//! let conn = ClientConnection::new(Arc::new(config), server_name).unwrap();
//! let tcp = TcpStream::connect(("www.googleapis.com", 443)).unwrap();
//! let mut stream = StreamOwned::new(conn, tcp);
//!
//! let auth = HttpAuthBearer::new("token");
//! let params = GcalEventsListParams {
//!     max_results: Some(10),
//!     single_events: true,
//!     ..Default::default()
//! };
//! let mut coroutine = GcalEventsList::new(&auth, "primary", &params).unwrap();
//!
//! let mut arg: Option<&[u8]> = None;
//! let mut buf = [0u8; 8192];
//! let mut read = Vec::new();
//!
//! let out = loop {
//!     match coroutine.resume(arg.take()) {
//!         GcalCoroutineState::Complete(Ok(out)) => break out,
//!         GcalCoroutineState::Complete(Err(err)) => panic!("{err}"),
//!         GcalCoroutineState::Yielded(GcalYield::WantsRead) => {
//!             let n = stream.read(&mut buf).unwrap();
//!             read.clear();
//!             read.extend_from_slice(&buf[..n]);
//!             arg = Some(&read);
//!         }
//!         GcalCoroutineState::Yielded(GcalYield::WantsWrite(bytes)) => {
//!             stream.write_all(&bytes).unwrap();
//!         }
//!     }
//! };
//!
//! for event in &out.response.items {
//!     println!("{:?}: {:?}", event.start, event.summary);
//! }
//! ```

extern crate alloc;
#[cfg(feature = "client")]
extern crate std;

pub mod coroutine;
pub mod v3;
