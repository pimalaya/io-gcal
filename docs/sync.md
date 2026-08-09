# Keeping a local copy in step

The Calendar API offers two mechanisms that are often confused, and that only work together: sync tokens tell a consumer *what* changed, push channels tell it *when* to ask. This note records how they compose, since neither the crate nor the reference states it in one place.

## The sync token loop

A first listing without a sync token is the baseline. Only the last page carries `nextSyncToken`; the pages before it carry `nextPageToken` instead, so a consumer must walk the whole listing to the end before it has a token worth storing.

Passing that token back on a later listing returns only what changed since. Deletions are part of the answer: an event comes back with the `cancelled` status, and a calendar list entry with its `deleted` flag set, whatever the `showDeleted` parameter says. The last page of the incremental listing carries a fresh token, which replaces the stored one.

The token encodes the shape of the listing that produced it, so most filters cannot be combined with it: the API rejects `q`, `iCalUID`, `orderBy`, `timeMin`, `timeMax`, `updatedMin`, `privateExtendedProperty` and `sharedExtendedProperty` alongside a sync token, and refuses an explicit `showDeleted=false`. Every other parameter, `singleEvents` in particular, must stay exactly as it was on the baseline, or the result is undefined. In practice this means a consumer decides once whether it stores series or instances, and never changes its mind without re-baselining.

The crate only puts `showDeleted` on the wire when it is true, so leaving `GcalEventsListParams::show_deleted` false is safe next to a sync token: the parameter is simply absent rather than explicitly false.

A token eventually expires, and the server answers HTTP 410 rather than degrading silently. `GcalSendError::is_sync_token_expired` recognises it. The only correct recovery is to drop the stored token, clear the local copy and run a full baseline again: the server no longer knows what the consumer missed.

## The push channel

A `watch` method opens a channel that POSTs a notification to a webhook whenever the watched collection changes. The notification carries no event data by default, only the fact that something moved on the resource, which is exactly what the sync token loop needs to be triggered. The two are complementary: the channel replaces polling, the token still does the fetching.

A channel expires on its own, so the receiver watches the `expiration` field and re-opens one before then; the API does not renew it. Closing a channel early goes through `channels.stop`, which needs both the channel id and the resource id returned when it was opened, so both must be stored, not just the id.

Channels also mean the crate ships no watcher coroutine. io-gmail owns a timer because Gmail's alternative to Pub/Sub is polling `users.history.list`; here the loop is driven by an inbound HTTP request the caller's own server receives, which is well outside what an I/O-free coroutine can model.

## Events that are neither created nor deleted

Two cases surprise consumers that treat `cancelled` as "remove it locally":

A cancelled *exception* of a series that is itself alive means that one instance should stop being shown, not that the series is gone. Such an event is only guaranteed to carry its id, its `recurringEventId` and its `originalStartTime`, and the consumer keeps it for as long as the parent series lives.

A cancelled event on the organizer's own calendar keeps its details, so that it can be restored. An incremental listing with `showDeleted` left false hides those details, which is why a consumer that wants to offer an undelete must ask for them explicitly.
