# Current implementation status

Repository: `JENN2046/VCPMobile-Native`; branch: `codex/native-rust-m0-m1`; tracker: Issue #1. Read latest Actions and Issue comments before treating this snapshot as current.

## Verified baseline

`dce262d03f36802e8c66f99953e9605f8fdeea14` / run `40299726811`: both platform jobs SUCCESS. Rust 92 tests (53+12+14+6+7) and strict clippy passed; Kotlin 4/4; Android instrumentation 13/13; iOS simulator 12/12 with zero failed/skipped/expected failures. The prior `5b65ae6` run40296683389 also passed but is not this batch. Scope remains synthetic TLS, cloud build and simulators, not physical devices or actual VCP service. Original failed runs retained.

## Current read-only Topic history UI batch (not yet verified)

The typed history reader and vault wrappers from `dce262d` have passed their own CI. This new batch attaches them to Android Compose/iOS SwiftUI: explicit Topic selection, manually fetched pages, context/view-generation checks, bounded plain-text previews, tombstones and max 400 in-memory rows. No background polling, disk cache, POST, task control, automatic message replay or AA completion inference. Topic switch, Origin edit, reconnect and local removal purge the view cache. Added unpaired-screen visibility assertions; existing tests and locked dependencies remain.

The new UI has **not** yet completed its own Rust/Android/iOS CI. See `docs/M2_HISTORY_VIEW.md`. No actual NativeSync service pairing/history acceptance or real-device validation is implied.

M1 baseline simulator criterion passed. M2 real NativeSync service pairing/history/submission/ACK-loss acceptance is still incomplete. M3–M5, physical phones and distribution signing remain incomplete. No production, main, old repository, real history or credentials modified.

## Next

Inspect this UI batch's exact CI and repair actual failures. Then implement exact original-pending receipt reconciliation and isolated real-service acceptance, keeping execution receipts separate from history display. Read the source map before changing protocol behavior. Do not repeat active CI or overwrite concurrent HEAD.

Earlier status entries remain in `docs/history/STATUS_THROUGH_6c6b4c4.md`; early “no source/no vault/no lock” statements are historical, not current.
