#![allow(dead_code)]

use io_gcal::coroutine::*;

pub fn run<C: GcalCoroutine<Yield = GcalYield>>(
    coroutine: &mut C,
    response: &[u8],
) -> (C::Return, Vec<u8>) {
    let mut written = Vec::new();
    let mut fed = false;
    let mut arg: Option<&[u8]> = None;

    loop {
        match coroutine.resume(arg.take()) {
            GcalCoroutineState::Complete(ret) => return (ret, written),
            GcalCoroutineState::Yielded(GcalYield::WantsWrite(bytes)) => {
                written.extend_from_slice(&bytes);
            }
            GcalCoroutineState::Yielded(GcalYield::WantsRead) => {
                if fed {
                    arg = Some(&[]);
                } else {
                    fed = true;
                    arg = Some(response);
                }
            }
        }
    }
}

pub fn json_response(status_line: &str, body: &str) -> Vec<u8> {
    http_response(
        status_line,
        &[
            ("Connection", "keep-alive"),
            ("Content-Type", "application/json"),
        ],
        body.as_bytes(),
    )
}

pub fn empty_response(status_line: &str) -> Vec<u8> {
    http_response(status_line, &[("Connection", "close")], &[])
}

fn http_response(status_line: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut response = Vec::new();
    response.extend_from_slice(status_line.as_bytes());
    response.extend_from_slice(b"\r\n");

    for (name, value) in headers {
        response.extend_from_slice(name.as_bytes());
        response.extend_from_slice(b": ");
        response.extend_from_slice(value.as_bytes());
        response.extend_from_slice(b"\r\n");
    }

    response.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
    response.extend_from_slice(body);
    response
}
