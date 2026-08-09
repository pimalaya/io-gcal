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

The suites under tests/ mirror the source tree, one file per resource plus the cross-cutting ones:

| Suite               | What it covers                                                                                     |
|---------------------|-----------------------------------------------------------------------------------------------------|
| tests/common/mod.rs | The scripted-coroutine harness: canned responses in, request bytes and terminal value out            |
| tests/send.rs       | The transport: the authorized request, the error envelope, the redirect refusal, the keep-alive flag  |
| tests/query.rs      | The query serializer: what reaches the URL and what deliberately does not                            |
| tests/acl.rs, calendar_list.rs, calendars.rs, channels.rs, colors.rs, events.rs, freebusy.rs, settings.rs | One file per resource: every method's request line, query parameters, body and parsed response, plus the validation each constructor applies and the wire spelling of every enum |
| tests/client.rs     | The std client over a scripted stream, including its error paths                                     |
| tests/google.rs     | The live tests, ignored by default                                                                   |

A test asserts on the request as written on the wire, since the query keys are camelCase and a silent rename is exactly the kind of regression the offline suite exists to catch.

tests/google.rs holds two ignored end-to-end tests against the live Calendar API, both needing a TLS feature and a `GCAL_ACCESS_TOKEN` environment variable. `account` only reads and is satisfied by the https://www.googleapis.com/auth/calendar.readonly scope; `calendar` walks the whole CRUD surface on a throwaway secondary calendar, needs the https://www.googleapis.com/auth/calendar scope, and deletes that calendar however the run ends:

```sh
GCAL_ACCESS_TOKEN=<token> cargo test --test google -- --ignored
```

The tests do no OAuth of their own, so any grant that yields a bearer token works.

The push channels are left out of the live tests: a watch needs a publicly reachable HTTPS webhook for Google to POST to, which a test process cannot provide.

## Minting a token without a human

A token from the authorization-code flow acts as you and expires within the hour, which rules out an unattended run. A service account instead signs its own assertion and trades it for a token whenever it needs one, the JWT-bearer grant, and tests/google.sh does that trade:

```sh
GCAL_ACCESS_TOKEN=$(./tests/google.sh key.json) cargo test --test google -- --ignored
```

The key file is the JSON one the Google Cloud console hands out under IAM -> Service Accounts -> Keys -> Add key. Nothing else is needed: no role, no domain-wide delegation, no Workspace domain. A service account is a Calendar principal in its own right and owns the calendars it creates, which is all the live tests touch, so no personal calendar is read or written.

That is also why the assertions in `account` check the shape of what comes back rather than its presence: a human account always has settings and a primary calendar, a fresh service account has neither.

To move the CI job off `workflow_dispatch` onto every push, store the key JSON as a secret and mint the token in the job with the same script. The variant worth preferring stores nothing at all: `google-github-actions/auth@v2` with Workload Identity Federation exchanges the GitHub OIDC token for a scoped access token, so no key ever leaves Google.

## Coverage

cargo-tarpaulin ships in the devshell, so the number CI reports can be reproduced locally:

```sh
cargo tarpaulin --engine llvm --out Stdout
```

The offline suites cover 97% of the lines. What is left out is `GcalClientStd::connect`, which opens a real TCP and TLS connection to www.googleapis.com and therefore cannot run offline, plus a handful of lines inside generic functions that the llvm instrumentation attributes to no test even though the suites demonstrably execute them. Do not reshape the code to chase those.

## CI

Three jobs run out of .github/workflows/tests.yml. The shared Pimalaya `tests` job builds and runs the offline suites on every push, and a `coverage` job reports tarpaulin's line coverage to Codecov.

The `google-tests` job runs the live tests on every push. It stores no access token, since one would expire within the hour: it stores the JSON key of a service account as the `GCAL_SERVICE_ACCOUNT_KEY` repository secret, and tests/google.sh trades that key for a fresh token at the start of each run. Setting it up once:

1. In the Google Cloud console, create a service account under IAM -> Service Accounts, with no role, then Keys -> Add key -> Create new key -> JSON.
2. Enable the Calendar API for the project, https://console.cloud.google.com/apis/library/calendar-json.googleapis.com, or every call answers 403 `accessNotConfigured`.
3. Paste the whole file, newlines included, into Settings -> Secrets and variables -> Actions -> New repository secret, named `GCAL_SERVICE_ACCOUNT_KEY`.

The job derives the token from the secret rather than reading it directly, so GitHub does not know to redact it; the run registers it with `::add-mask::` before anything can log it, which matters because the job runs at `RUST_LOG=trace`. The job is also guarded on the repository name, since a fork holds no secret and would fail on every contributor's push.

A JSON key is a long-lived credential sitting in two places, GitHub and whatever machine downloaded it. Deleting the local copy once the secret is set, and disabling the key in the console if it ever leaks, is the whole of the hygiene. The way to avoid the credential entirely is Workload Identity Federation: `google-github-actions/auth@v2` exchanges the GitHub OIDC token for a scoped access token, so nothing is stored on either side. That swap is a drop-in replacement for the minting step, and worth making once this is green.

## Checking the API surface against the reference

The Calendar API publishes a machine-readable discovery document, which is the source of truth for the resources, methods, query parameters and schemas the crate mirrors:

```sh
curl https://www.googleapis.com/discovery/v1/apis/calendar/v3/rest
```

Any addition to the crate should be checked against it rather than against memory of the HTML reference.
