# M2 native confirmed original message UI (2026-10-27)

This batch exposes the existing Rust/UniFFI `NativePendingJournal.send_original_message_once`
through deliberate Compose and SwiftUI message composers. It does not add a
new server API or an automatic send/retry path.

## Explicit user flow

1. Pair/restore the exact HTTPS origin, read authorized session/Topic and
   deliberately select the Topic. No request on app open or Topic selection.
2. Enter an in-memory text draft (max 4,000 UTF-16 units, not stored in
   preferences or logs), press Send, then explicitly confirm a modal warning.
   A fresh random UUID request ID is generated exactly once per invocation.
3. Platform vault loads an origin-scoped protected credential with its epoch.
   The existing Rust method rechecks a real HTTPS session and Topic, reserves
   one local SQLite pending row and atomically claims the first attempt before
   making a single no-retry, no-redirect POST.
4. The vault checks its credential epoch after the call. UI checks its
   origin, entry/session, Topic and view generation before presenting results.
   A changed epoch **cannot retract an already claimed POST**. Treat any
   ambiguous/cancelled/stale outcome as UNKNOWN and inspect the preserved
   pending journal. HTTP202 is only HTTP_202_UNVERIFIED, not AA completion.
5. After ANY attempted send, the UI disables further sends until the user
   explicitly reads a fresh authenticated pending inventory. If the local
   Topic journal is still nonempty, sending remains disabled. No automatic
   reconciliation, retry, row cleanup, or cross-session task control.
   Any new send is still subject to Rust transactional single-pending rule.

## Evidence and limits

Added read-only negative UI assertions that unpaired screens never expose
send or draft controls. Previous UI, vault, Rust, TLS and lockfile tests remain.
No actual NativeSync server, real Cookie, paid model, live user message,
ACK-loss, Android/iPhone device or distribution signing was used in this batch.
Cloud CI must pass on its own exact commit before calling the source compiled.
Next: explicit receipt reconciliation action and isolated R2.1 service tests;
then M3 real conversation/status wiring. iOS device file protection and signing
remain **unverified on physical iPhone** until a device and authorized signing
are available.
