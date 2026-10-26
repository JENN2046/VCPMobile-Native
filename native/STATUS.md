# Current implementation status

Repository: `JENN2046/VCPMobile-Native`; branch: `codex/native-rust-m0-m1`; tracker: Issue #1. Read latest Actions and Issue comments before treating this snapshot as current.

## Verified baseline

`238f450f4dbf31d69c2faeec7a2f0a6c900afb7f` / run `40309677622`: Android and iOS jobs SUCCESS, including Rust, Swift/Kotlin bridge and both simulators. This validates the read-only Topic UI cloud batch, not real service or physical devices.

`dce262d03f36802e8c66f99953e9605f8fdeea14` / run `40299726811`: both platform jobs SUCCESS. Rust 92 tests (53+12+14+6+7) and strict clippy passed; Kotlin 4/4; Android instrumentation 13/13; iOS simulator 12/12 with zero failed/skipped/expected failures. The prior `5b65ae6` run40296683389 also passed but is not this batch. Scope remains synthetic TLS, cloud build and simulators, not physical devices or actual VCP service. Original failed runs retained.

## Read-only Topic history UI batch (verified on 238f450)

The typed history reader and vault wrappers from `dce262d` have passed their own CI. This new batch attaches them to Android Compose/iOS SwiftUI: explicit Topic selection, manually fetched pages, context/view-generation checks, bounded plain-text previews, tombstones and max 400 in-memory rows. No background polling, disk cache, POST, task control, automatic message replay or AA completion inference. Topic switch, Origin edit, reconnect and local removal purge the view cache. Added unpaired-screen visibility assertions; existing tests and locked dependencies remain.

The read-only UI passed its own Rust/Android/iOS cloud jobs in run `40309677622` (both SUCCESS). See `docs/M2_HISTORY_VIEW.md`. No actual NativeSync service pairing/history acceptance or real-device validation is implied.
## Read-only Topic history UI baseline

The typed history reader and vault wrappers from `dce262d` have passed their own CI. This new batch attaches them to Android Compose/iOS SwiftUI: explicit Topic selection, manually fetched pages, context/view-generation checks, bounded plain-text previews, tombstones and max 400 in-memory rows. No background polling, disk cache, POST, task control, automatic message replay or AA completion inference. Topic switch, Origin edit, reconnect and local removal purge the view cache. Added unpaired-screen visibility assertions; existing tests and locked dependencies remain.

`238f450f4dbf31d69c2faeec7a2f0a6c900afb7f` / run40309677622 is now completed/success on both platforms. The independent receipt-query input `be16729300a8782021c07ca1cefa56e4f3301baa` / run40308456032 also passed both jobs. These are distinct exact inputs, not a combined PASS. See `docs/M2_HISTORY_VIEW.md`. No actual NativeSync service pairing/history acceptance or real-device validation is implied.

## Independent native recovery wiring batch

Both inputs are integrated without replacing history UI. UniFFI pending journal, protected-vault recovery wrappers and explicit query/cancel UI are implemented. Original/current session identities remain separate; expiry, credential fingerprint/epoch, Topic and view fences precede exact one-use receipt consumption. Queries and cancellation never resubmit. Local storage is OS-protected and backup-excluded, with credentials kept outside SQLite. See `docs/M2_NATIVE_RECOVERY_WIRING.md`.

Local Rust96 and strict clippy, real bindings, Kotlin host6 and Android application/instrumentation source compilation are checked independently. This batch still needs its exact combined cloud run for Swift/iOS and both simulators; no earlier pass is inherited. The UI has no new send/streaming path and actual NativeSync/device acceptance remains separate.

M1 baseline simulator criterion passed. M2 real NativeSync service pairing/history/submission/ACK-loss acceptance is still incomplete. M3–M5, physical phones and distribution signing remain incomplete. No production, main, old repository, real history or credentials modified.

## New Rust pending receipt gate (this revision not yet CI-verified)

Composed existing secure Cookie HTTPS session and Topic reads with original first-attempt SQLite record and exact receipt oracle, requiring a fresh session before and after history GET. Four synthetic gate tests added. No UniFFI/mobile wiring, no actual NativeSync service or ACK-loss acceptance; see `docs/M2_RECEIPT_GATE.md`.

## Next

Verify exact CI for this new gate, fix real failures, then wire explicit native recovery and isolate an actual NativeSync service. Do not repeat active CI or overwrite concurrent HEAD.
Inspect this independent recovery batch's own exact CI before integration. Coordinate with the shared-branch owner; do not overwrite newer HEAD or repeat active CI. The remaining real-service/physical-device chain, explicit send/streaming and result UI require their own evidence, with execution receipts separate from AA completion.

Earlier status entries remain in `docs/history/STATUS_THROUGH_6c6b4c4.md`; early “no source/no vault/no lock” statements are historical, not current.


## M2 native pending inventory (new revision, CI not yet verified)

Based on `60226d76e9d608d7e5e4784373c36f1230a2341f` / run40319359924,
whose two jobs passed (Rust 57+12+14+6+7 = 96, Kotlin 4, Android emulator
13, iOS simulator 12; zero simulator failures/skips). These are
cloud/synthetic results for the **parent revision**, not proof of this batch.

Added a per-origin app-private SQLite pending journal and explicitly triggered
read-only inventory to both native Topic views. It requires a live authenticated
session GET and the displayed exact entry/session/Topic; platform epoch and
view generation are checked after the await. UI gets request identity/state
only, no private payload bytes. No POST, replay, pending deletion, task control,
background polling or AA-completion inference. Added four offline Rust tests
and tightened both unpaired UI guards. Existing assertions/locked dependencies
remain. See `docs/M2_PENDING_JOURNAL.md`.

**This revision has no own Rust/mobile CI PASS yet.** Do not reuse parent results.
Next verify exact Actions and repair concrete failures; then wire the original
one-use native submission, epoch-fenced explicit receipt confirmation and
isolated real NativeSync service acceptance. M3–M5/device/signing pending.

## M2 original initial message dispatch seam (unverified new revision)

A Rust/UniFFI-only, user-invoked `NativePendingJournal.send_original_message_once`
now composes fresh authorized session GET, original scoped/transactional message
reservation, atomic first-attempt claim and existing exact guarded HTTPS POST.
All HTTP results preserve pending; 202 is unverified. Two offline tests added.
No Android/iOS send UI is wired, no live NativeSync service test exists.
This candidate requires its own exact Actions Rust/Android/iOS validation.
See `docs/M2_NATIVE_FIRST_SEND.md`.


## Android chat local candidate

Shared `286f819875856d2719580b1d576b7c1678d245e1` / run40346817148
finished both jobs SUCCESS. This independent local candidate combines that
first-send seam, private inventory and second-session receipt gate with
`e5e5b04` recovery fences. One journal/store/claim/receipt inspector is used.
Android explicit send is fenced under the secure vault gate; bounded reply
reception uses existing history GET, not an unverified token-stream protocol.
Local Rust109, clippy, release/bindings, Android Kotlin compile and 3 JVM
history-window tests passed. No new remote CI, complete APK, simulator, real
NativeSync, model or physical-device acceptance is claimed. Library material
transfer failed normally; no bypass. See docs/ANDROID_CHAT_CANDIDATE.md for
exact scope, overlap with the other owner and remaining source/service/device
conditions; docs/ANDROID_CHAT_DEVICE_ACCEPTANCE.md is the unexecuted real plan.
iOS Complete protection: PENDING_REAL_DEVICE_ACCEPTANCE, never passed; original
assertions/failure evidence retained. No M2/M5 completion or shared/main merge.
