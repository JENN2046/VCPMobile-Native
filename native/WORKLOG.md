# Native client work log

Repository: JENN2046/VCPMobile-Native, development branch codex/native-rust-m0-m1; Issue #1 is the shared work tracker. Read the latest exact HEAD and Actions before acting.

## Preserved history

All earlier entries are retained byte-for-byte in `docs/history/WORKLOG_THROUGH_6c6b4c4.md`. Earlier failures, source recovery and migration remain auditable; historical statements are not current state.

## Keychain repair verified

`6c6b4c4700e2f06358431dd0fe7f90cad6835619` / run `40290030154`: Android and iOS jobs SUCCESS. iOS artifact12635887933 SHA256 `b32b5fb5be7d4a3e91cb7237ccefa19bdbb11cf4c5f06b900e24ece9997ecc80` contains 9 passed, 0 failed/skipped/expected failures. All six real Keychain tests and original three UI tests passed. CRC and 60 scoped source hashes verified. Original failing run40287260542 remains evidence. No production signing credentials were used.

## M2 pairing batch

Implemented source-backed, one-use HTTPS pairing with exact Origin, 32-character code, no existing Cookie/Authorization, no redirects or implicit retries. Strict response validation returns an opaque credential. Save in platform vault before querying session; failed GET is SAVED_SESSION_UNCONFIRMED, never another pairing POST. Keep original expiry and reject late results after session replacement/removal.

Added SwiftUI and Compose connection screens for pairing, explicit read-only restore and local removal. Only the non-secret last origin is stored in ordinary settings; codes are ephemeral and cleared on submission. Changing origin clears displayed Topic metadata. Screens do not initiate networking on launch. Existing diagnostic views and all original tests remain.

Eight Rust tests added, including five real loopback TLS scenarios in a synthetic fixture; two offline connection UI tests per platform added. Local Python exercised all five fixture responses with CA and hostname verification; shell/Python/Swift parsing and original test/lock identities checked. This environment has no Rust/Android/Xcode toolchains, so new Rust and platform tests await their own CI; predecessor passes are not inherited.

Actual NativeSync service pairing, Topic history, message submission/reconciliation and ACK-loss acceptance remain incomplete. See `docs/M2_PAIRING.md`. No backend, production, old repository, main, real history or secrets changed. Final commit/run and next actual outcome are recorded in Issue #1.
