#!/usr/bin/env bash
#
# Mints a Google Calendar access token from a service account key and
# prints it on stdout, so the live tests can run unattended:
#
#   GCAL_ACCESS_TOKEN=$(./tests/google.sh key.json) \
#   cargo test --test google -- --ignored
#
# A service account authenticates with the JWT-bearer grant: it signs a
# short-lived assertion with its private key and trades it for an access
# token, no human and no refresh token in the loop. That is what makes
# the live tests runnable from CI, where the hour-long token of the
# authorization-code flow is stale before the job starts.
#
# The key file is the JSON one the Cloud console hands out under IAM ->
# Service Accounts -> Keys -> Add key. The service account needs no role
# and no domain-wide delegation: it owns its own calendars, which is all
# the tests touch.
#
# The token reaches stdout and nothing else: keep it out of shell
# history and CI logs.

set -euo pipefail

key="${1:-}"
scope="${2:-https://www.googleapis.com/auth/calendar}"

if [ -z "$key" ]; then
    echo "usage: ${0##*/} <service-account-key.json> [scope]" >&2
    exit 2
fi

if [ ! -r "$key" ]; then
    echo "cannot read the service account key at \`$key\`" >&2
    exit 2
fi

client_email=$(jq -r '.client_email // empty' "$key")
private_key=$(jq -r '.private_key // empty' "$key")
token_uri=$(jq -r '.token_uri // "https://oauth2.googleapis.com/token"' "$key")

if [ -z "$client_email" ] || [ -z "$private_key" ]; then
    echo "\`$key\` is not a service account key (no client_email or private_key)" >&2
    exit 2
fi

# base64url, the encoding a JWT uses: the standard alphabet with the two
# URL-unsafe characters swapped and the padding dropped.
b64url() {
    openssl base64 -e -A | tr '+/' '-_' | tr -d '='
}

now=$(date +%s)
header=$(printf '{"alg":"RS256","typ":"JWT"}' | b64url)
claims=$(printf '{"iss":"%s","scope":"%s","aud":"%s","iat":%s,"exp":%s}' \
    "$client_email" "$scope" "$token_uri" "$now" "$((now + 3600))" | b64url)
signature=$(printf '%s.%s' "$header" "$claims" |
    openssl dgst -sha256 -sign <(printf '%s' "$private_key") -binary | b64url)

response=$(curl -sS "$token_uri" \
    -d grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer \
    --data-urlencode "assertion=$header.$claims.$signature")

token=$(printf '%s' "$response" | jq -r '.access_token // empty')

if [ -z "$token" ]; then
    echo "the token endpoint refused the assertion: $response" >&2
    exit 1
fi

printf '%s\n' "$token"
