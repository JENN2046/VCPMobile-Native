# NativeSync R2.1 submission flow boundary

This is a local Rust M2 component, not a network-connected mobile client. Its source is the accepted cumulative R2.1 portal patch plus blob-verified portal.js, connection.js and submissionRecovery.js recorded in NATIVESYNC_R21_SOURCE_MAP.md.

## Implemented

Session and history GET request plans are restricted to an exact canonical DNS HTTPS origin, authorized Topic and JS-safe history cursor. New first-attempt planning requires a verified /mobile/api/session response, source-defined requestId/text/attachments or requestId/agentId validation, preserved original JSON bytes, and matching entry/session fingerprint guards. SQLite reserves at most one unresolved request per entry/Topic, across sessions and kinds. A conditional transition from prepared to awaiting_confirmation atomically permits only one initial dispatch intent even across separate SQLite connections.

Whenever storage fails, payload is invalid, Topic scope changes or the local claim is already taken, callers get an error and must NOT send. An interrupted first attempt remains unknown, including when the interruption preceded actual network dispatch. There is no autonomous resend.

An authenticated Topic history read can be used to reconcile pending. A unique matching recovery receipt closes the original pending record, not the AA task. Missing same-session receipt requires a separate explicit retry decision and rechecking the CURRENT server context. Missing cross-session receipt or ambiguous/mismatched receipt remains for review. Reading a task across sessions grants no authority to cancel, resolve or execute it.

## Deliberately not yet implemented

No actual HTTPS client, platform Keychain/Keystore Cookie storage, session pairing, redirect policy, status 202/ACK handling, authenticated GET provenance, attachment streaming, UI send pathway or real remote recovery test is wired. Native networking MUST use the exact HTTPS origin with no redirect, enforce a host/path-scoped __Secure-vcp_mobile Cookie from protected platform storage, require the exact Origin header on writes, and include both reject-only X-NativeSync-Entry-Id and X-NativeSync-Session-Marker headers. No cookie may be logged or persisted in PendingStore.

An outbound plan is not a successful POST or proof of server acceptance. The next work package must implement host transport, safe authentication, bounded responses, and real isolated service fixtures before M2 acceptance.

## Evidence

The base M1 apps first met the actual cloud emulator criterion at f678b1f / run40236267942: Android 4/4, iOS 3/3. This new local-flow code requires its own CI run and does not inherit those PASS results. No production modification or private installation ZIP publication.
