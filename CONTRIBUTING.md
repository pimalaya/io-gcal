# Contributing guide

Thank you for investing your time in contributing to I/O Google Calendar.

Whether you are a human or an AI agent, read these in order before touching the code:

1. the [Pimalaya README](https://github.com/pimalaya) for what the project is and how its repositories stack;
2. the [Pimalaya CONTRIBUTING](https://github.com/pimalaya/.github/blob/master/CONTRIBUTING.md) guide, which chains to the shared architecture and guidelines;
3. the inline header documentation, starting with src/lib.rs: it is the architecture document of this crate;
4. the docs/ folder for the development history and living plans.

Everything below documents only what differs from the Pimalaya standards.

## Feature matrix

io-gcal follows the standard layered split (I/O-free coroutines, then the std client behind the `client` feature), plus a vendored switch compiling the TLS dependencies from source:

```sh
cargo build --no-default-features                        # coroutines only, no std leak
cargo build --no-default-features --features client      # light client, no TLS deps
cargo build                                              # full client (rustls-ring by default)
cargo build --no-default-features --features rustls-aws  # full client, aws-lc-rs crypto
cargo build --no-default-features --features native-tls  # full client, platform TLS
cargo build --features vendored                          # vendored TLS dependencies
```

## Tests

The default suite is fully offline: every coroutine is driven against an in-memory stream replaying a canned HTTP response, so no network access nor OAuth token is required.

```sh
cargo test
```

tests/gcal.rs is an ignored end-to-end test walking the whole CRUD surface against the live Calendar API. It needs a TLS feature and a `GCAL_ACCESS_TOKEN` environment variable with the https://www.googleapis.com/auth/calendar scope. It works on a throwaway secondary calendar and deletes it at the end:

```sh
GCAL_ACCESS_TOKEN=<token> cargo test --test gcal -- --include-ignored
```

## Checking the API surface against the reference

The Calendar API publishes a machine-readable discovery document, which is the source of truth for the resources, methods, query parameters and schemas the crate mirrors:

```sh
curl https://www.googleapis.com/discovery/v1/apis/calendar/v3/rest
```

Any addition to the crate should be checked against it rather than against memory of the HTML reference.
