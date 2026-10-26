# Current implementation status

Repository: `JENN2046/VCPMobile-Native`; branch: `codex/native-rust-m0-m1`; tracker: Issue #1. Read latest Actions and Issue comments before treating this snapshot as current.

## Verified baseline

`dce262d03f36802e8c66f99953e9605f8fdeea14` / run `40299726811`: both platform jobs SUCCESS. Rust 92 tests (53+12+14+6+7) and strict clippy passed; Kotlin 4/4; Android instrumentation 13/13; iOS simulator 12/12 with zero failed/skipped/expected failures. The prior `5b65ae6` run40296683389 also passed but is not this batch. Scope remains synthetic TLS, cloud build and simulators, not physical devices or actual VCP service. Original failed runs retained.

## Read-only Topic history UI baseline

The typed history reader and vault wrappers from `dce262d` have passed their own CI. This new batch attaches them to Android Compose/iOS SwiftUI: explicit Topic selection, manually fetched pages, context/view-generation checks, bounded plain-text previews, tombstones and max 400 in-memory rows. No background polling, disk cache, POST, task control, automatic message replay or AA completion inference. Topic switch, Origin edit, reconnect and local removal purge the view cache. Added unpaired-screen visibility assertions; existing tests and locked dependencies remain.

`238f450f4dbf31d69c2faeec7a2f0a6c900afb7f` / run40309677622 is now completed/success on both platforms. The independent receipt-query input `be16729300a8782021c07ca1cefa56e4f3301baa` / run40308456032 also passed both jobs. These are distinct exact inputs, not a combined PASS. See `docs/M2_HISTORY_VIEW.md`. No actual NativeSync service pairing/history acceptance or real-device validation is implied.

## Independent native recovery wiring batch

Both inputs are integrated without replacing history UI. UniFFI pending journal, protected-vault recovery wrappers and explicit query/cancel UI are implemented. Original/current session identities remain separate; expiry, credential fingerprint/epoch, Topic and view fences precede exact one-use receipt consumption. Queries and cancellation never resubmit. Local storage is OS-protected and backup-excluded, with credentials kept outside SQLite. See `docs/M2_NATIVE_RECOVERY_WIRING.md`.

Local Rust96 and strict clippy, real bindings, Kotlin host6 and Android application/instrumentation source compilation are checked independently. This batch still needs its exact combined cloud run for Swift/iOS and both simulators; no earlier pass is inherited. The UI has no new send/streaming path and actual NativeSync/device acceptance remains separate.

M1 baseline simulator criterion passed. M2 real NativeSync service pairing/history/submission/ACK-loss acceptance is still incomplete. M3–M5, physical phones and distribution signing remain incomplete. No production, main, old repository, real history or credentials modified.

## Next

Inspect this independent recovery batch's own exact CI before integration. Coordinate with the shared-branch owner; do not overwrite newer HEAD or repeat active CI. The remaining real-service/physical-device chain, explicit send/streaming and result UI require their own evidence, with execution receipts separate from AA completion.

Earlier status entries remain in `docs/history/STATUS_THROUGH_6c6b4c4.md`; early “no source/no vault/no lock” statements are historical, not current.
