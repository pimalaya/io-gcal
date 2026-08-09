//! Shared helpers for the offline integration suites.
//!
//! Every Calendar coroutine is a single HTTP exchange, so the whole
//! harness is three steps: resume once to collect the request bytes,
//! resume again to acknowledge the read request, then feed the canned
//! response and read the terminal value. [`expect_exchange`] chains the
//! three and is what nearly every test calls; the individual `expect_*`
//! steps are there for the tests that assert on the intermediate
//! states, following the io-imap canonical layout.
//!
//! Each suite compiles this module on its own and uses a subset of it,
//! so the rest ends up flagged as dead code; suppress the noise at the
//! module level.

#![allow(dead_code)]

use core::fmt::Debug;

use io_gcal::coroutine::*;
use io_http::rfc6750::bearer::HttpAuthBearer;

/// The bearer credential every offline test authenticates with.
pub fn auth() -> HttpAuthBearer {
    HttpAuthBearer::new("fake-token")
}

/// Serializes an HTTP/1.1 response: the given status line, the extra
/// headers, a correct `Content-Length` and the body.
pub fn http_response(status: &str, extra: &[(&str, &str)], body: &str) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status}\r\n");

    for (name, value) in extra {
        out.push_str(&format!("{name}: {value}\r\n"));
    }

    out.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
    out.into_bytes()
}

/// Shortcut for a JSON [`http_response`] on a reusable connection.
pub fn json_response(status: &str, body: &str) -> Vec<u8> {
    http_response(
        status,
        &[
            ("Connection", "keep-alive"),
            ("Content-Type", "application/json"),
        ],
        body,
    )
}

/// Shortcut for a bodiless [`http_response`], as returned by the
/// delete, clear and stop methods.
pub fn empty_response(status: &str) -> Vec<u8> {
    http_response(status, &[("Connection", "keep-alive")], "")
}

/// Resumes a coroutine and returns the request bytes it wants written.
pub fn expect_wants_write<C, R>(coroutine: &mut C, arg: Option<&[u8]>) -> Vec<u8>
where
    C: GcalCoroutine<Yield = GcalYield, Return = R>,
    R: Debug,
{
    match coroutine.resume(arg) {
        GcalCoroutineState::Yielded(GcalYield::WantsWrite(bytes)) => bytes,
        state => panic!("expected WantsWrite, got {state:?}"),
    }
}

/// Resumes a coroutine, expecting it to ask for the response bytes.
pub fn expect_wants_read<C, R>(coroutine: &mut C)
where
    C: GcalCoroutine<Yield = GcalYield, Return = R>,
    R: Debug,
{
    match coroutine.resume(None) {
        GcalCoroutineState::Yielded(GcalYield::WantsRead) => {}
        state => panic!("expected WantsRead, got {state:?}"),
    }
}

/// Feeds `reply` to a coroutine and returns its terminal value.
pub fn expect_complete<C, R>(coroutine: &mut C, reply: &[u8]) -> R
where
    C: GcalCoroutine<Yield = GcalYield, Return = R>,
    R: Debug,
{
    match coroutine.resume(Some(reply)) {
        GcalCoroutineState::Complete(ret) => ret,
        state => panic!("expected Complete, got {state:?}"),
    }
}

/// Runs a whole exchange: returns the request as written on the wire,
/// then the terminal value the canned `reply` produces.
///
/// The request keeps its original casing, since the query keys the
/// Calendar API expects are camelCase and the assertions are about
/// exactly that.
pub fn expect_exchange<C, R>(coroutine: &mut C, reply: &[u8]) -> (String, R)
where
    C: GcalCoroutine<Yield = GcalYield, Return = R>,
    R: Debug,
{
    let bytes = expect_wants_write(coroutine, None);
    let request = String::from_utf8_lossy(&bytes).into_owned();
    expect_wants_read(coroutine);
    (request, expect_complete(coroutine, reply))
}

/// Returns the request line of a request, without the trailing
/// protocol version.
pub fn request_line(request: &str) -> &str {
    let line = request.lines().next().unwrap_or_default();
    line.strip_suffix(" HTTP/1.1").unwrap_or(line)
}

/// Returns the body of a request, everything past the header block.
pub fn request_body(request: &str) -> &str {
    request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_default()
}

/// Asserts that a request carries the given query pair, naming the
/// whole request line when it does not.
pub fn assert_query(request: &str, pair: &str) {
    let line = request_line(request);
    assert!(line.contains(pair), "expected `{pair}` in `{line}`");
}

/// Asserts that a request carries no such query key at all.
pub fn assert_no_query(request: &str, key: &str) {
    let line = request_line(request);
    assert!(!line.contains(key), "unexpected `{key}` in `{line}`");
}
