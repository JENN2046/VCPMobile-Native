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

## Pairing batch verified and history-read candidate integrated

`5b65ae66511c2da0e6459179f5f0115cc92c3f05` / run40296683389 completed successfully in both jobs. Android artifact12639072667 SHA256 `96cc3871b7cc5a19e6768f2f251eefc41aa563850de3b5c81dba7fe9a0528438`: Rust81, Kotlin4, Android12 passed. iOS artifact12639516488 SHA256 `b92c867813362bbfa93fca9880e34a1f53e12dd682585e30844b356824586137`: simulator11 passed, zero failures/skips/expected failures. ZIP CRC and the 70-file source snapshot were verified. The prior startup failure is retained, not deleted.

Reused the previously saved history-read candidate ZIP (SHA256 `2fb1834b2607272e3b8b0598529fe702f1ba823596e974ae430fa21058152a52`) rather than rewriting it. Its exact 10-file patch applied to the verified 5b65ae6 artifact; manifest before/after identities matched. Reviewed the unchanged source-backed history route, whole-page validation, authenticated context check, epoch fencing and no-dispatch boundary. Updated current notes separately.

The candidate adds 11 Rust tests and one unpaired-history guard per platform. Local patch/whitespace/shell and Swift parsing checks are not Rust/mobile test evidence. The new revision requires its own actual CI; no application history screen, live NativeSync acceptance or message execution is claimed. No private installation ZIP, credential, production data, dependency change, main/old-repository write or signing secret is included.

## Read-only history UI candidate after dce262d

`dce262d03f36802e8c66f99953e9605f8fdeea14` / run40299726811: Android/iOS jobs SUCCESS. Local artifact inspection found Rust 92 passed, strict clippy success, Kotlin4, Android13 instrumentation and iOS12 simulator all passed with no failures/skips. Both artifacts' CRC and digest are checked against the Actions metadata. This validates the source-backed read contract and platform preflight; not an actual NativeSync service.

A new unverified UI batch builds on it: manual Topic pick/read, 200-row paging via the existing credential vault, 400-row view-only cache, generation invalidation, safe native text previews and tombstones; execution receipts not rendered as completed, no dispatch or pending cleanup. The UI's own CI must be inspected before marking it passed. The original source ZIP is not published here; standard cloud artifacts are synthetic and bounded. See docs/M2_HISTORY_VIEW.md.

## Authenticated original pending receipt gate candidate after 238f450

Run40309677622 at 238f450: both Android and iOS CI jobs SUCCESS, cloud/synthetic UI evidence only. Built an internal Rust-only authenticated reconciliation composition: checked claimed original pending SQLite row, scoped Cookie session GET, authorized Topic history GET and a second session authority check before applying exact original receipt. Four offline gate tests added; all missing, cross-session absent, ambiguous, malformed or changed-authority outcomes preserve unknown state without POST/retry or task control. See docs/M2_RECEIPT_GATE.md. This candidate has NOT passed its own new CI or actual NativeSync service/ACK-loss acceptance. Next: run accurate CI, then explicit mobile per-origin store/epoch integration and isolated service tests.


## M2 native explicit pending inventory candidate (after 60226d)

Parent `60226d76e9d608d7e5e4784373c36f1230a2341f` /
run40319359924 completed both jobs SUCCESS. Independently read original
Android evidence archive (74-file source manifest matches its exact commit;
Rust96/Kotlin4/Android13) and iOS artifact (simulator12/12, 0 fail/skip).
Synthetic cloud run only; not real NativeSync service or physical device.

This candidate implements origin-scoped durable SQLite journal access and
read-only native recovery inventory in existing Android/iOS connection views.
Only deliberate user action starts a fresh authenticated session GET, and
old credential/view generations refuse late display results. No original
payload returns to the UI. State remains local UNKNOWN until the separate
authenticated exact-receipt gate confirms request identity; no POST or
removal occurs here. Four new Rust offline tests and strengthened unpaired
UI absence assertions; existing vault/bridge tests, Cargo.lock and TLS
checks retained. See docs/M2_PENDING_JOURNAL.md.

Do not mark this candidate accepted until its own Actions jobs/artifacts pass.
Next implement actual initial message POST with per-origin durable pending
identity and explicit recovery, then isolated real service/ACK-loss acceptance.

## Native first send seam after 072adcb

Parent `072adcb8fa6e68e4189fddfacad3b79cfdcae57e` / run40337872777
finished both Android and iOS jobs SUCCESS. This is parent cloud/synthetic
validation, not the next batch PASS. The next revision adds explicit Rust-only
initial native message POST composition with durable per-origin claim and
response categories that retain pending, plus two offline tests; mobile UI
and actual isolated NativeSync/ACK-loss acceptance are still pending.
See docs/M2_NATIVE_FIRST_SEND.md.
