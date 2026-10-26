# Current implementation status

Repository: `JENN2046/VCPMobile-Native`; branch: `codex/native-rust-m0-m1`; tracker: Issue #1. Read latest Actions and Issue comments before treating this snapshot as current.

## Verified baseline

`5b65ae66511c2da0e6459179f5f0115cc92c3f05` / run `40296683389`: both platform jobs SUCCESS. Rust 81 tests and strict clippy passed; Kotlin 4/4; Android instrumentation 12/12; iOS simulator 11/11 with zero failed/skipped/expected failures. Scope is synthetic TLS, cloud build and simulators, not physical devices or actual VCP service. Both original failed runs40287260542 and40295036441 remain evidence.

## This development batch, awaiting its own CI

Reviewed and integrated the existing history-read candidate against its exact base. Typed pages validate the whole 200-row envelope, monotonic safe cursors, unique message IDs and tombstones before returning any result. SessionCredential rechecks the authenticated session before a scoped history GET. Both platform vaults fence credential replacement/removal. HTTP401/403 remain explicit failures. No pending record is removed, no message is sent and no task completion is inferred.

Eleven Rust tests plus one unpaired-history guard per platform are added. All existing assertions and the Cargo lock remain unchanged. Actual new compile, binding and mobile results must come from this batch's own run; predecessor PASS is not inherited. See `docs/M2_HISTORY_READ.md`.

M1 baseline simulator criterion passed. M2 real NativeSync service pairing/history/submission/ACK-loss acceptance is still incomplete. M3–M5, physical phones and distribution signing remain incomplete. No production, main, old repository, real history or credentials modified.

## Next

Inspect this batch's exact CI. Then connect read-only history display/cache and exact original-pending receipt reconciliation, with real-service acceptance kept separate. Read the source map before changing protocol behavior. Do not repeat active CI or overwrite concurrent HEAD.

Earlier status entries remain in `docs/history/STATUS_THROUGH_6c6b4c4.md`; early “no source/no vault/no lock” statements are historical, not current.
