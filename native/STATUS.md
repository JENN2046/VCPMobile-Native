# Implementation status

Current scope: continue R1.1 M0–M5 on `codex/native-rust-m0-m1`, no repeated milestone approval. No main/PR/merge/release, production/history/account/signing changes or paid models.

## Observed evidence and current work

- `64d66cad618c7338cd07c5ccbcf49f1f5b4641e6`, run40216459025: Rust12 and Swift host/Apple target passed; Kotlin JUnit initialization failed, Android target skipped.
- `7c2977f1105aa961922c1b3312a56e69eeb3faa0`, run40221085134: Rust and host bindings passed; Kotlin4/4; Android APK built, UI3/4 (cancel timeout); iOS simulator3/3 passed. Not physical-device evidence.
- `8f907587e4d952efc63df4b5e25fa18e51659af6`, run40222103042: Linux core/bridge and Android build passed at the last observation; Android UI still in progress then. macOS failed BEFORE Rust compilation because Bash3.2 treated an empty array under nounset as unbound. This is not a failed SQLite assertion. Preserve the actual final run result separately.
- This revision removes the empty-array construct, generates a missing lock once then uses --locked for core operations, and archives bounded synthetic core logs plus exact tracked native source. Shell control-flow was locally tested with synthetic commands for existing/missing lock and cargo-test failure; this is not real local Rust compilation.
- Source packaging locally checked tracked-only scope, separately marked generated lock, exact tracked lock, and refusal on a modified tracked lock. No untracked files, credentials or runtime history included.
- Exact NativeSync materials FOUND, verified and partly reconstructed; see docs/NATIVESYNC_R21_SOURCE_MAP.md. Do not continue reporting the ZIP as unavailable.
- Added the source-grounded recovery fingerprint/receipt port and six tests, including one 50-case oracle. This revision needs its own compilation/tests; no inherited PASS.

## Milestones

M0 partial: accepted cumulative materials and exact recovery/client source available; full native HTTP/authentication and isolated service setup still to read/assemble.
M1 in verification: actual Compose/SwiftUI apps exist and have distinct simulator evidence. Require latest claimed revision to pass both suites.
M2 in implementation: bounded SQLite plus source-grounded receipt logic; no authenticated native transport yet. Topic-level pending admission/integration is still needed.
M3–M5 not complete. UI remains explicitly diagnostic. Real source, host tests, target libraries, apps, simulator, physical device and signing remain separate levels.

## Continuation

Hourly task reads live HEAD/CI before each bounded code batch. In-progress CI means defer rather than cancel or disable. Workflow no longer cancels an active run when a new commit arrives. Standard public-repo runners only; no cache/account/billing/secret changes; synthetic artifacts retain1day. Packaging is a checkpoint, not M5 acceptance.

Next: consume actual CI, fix real failures, retrieve and commit the newly resolved lock, assemble exact portal/auth contract using the recovered materials, then implement and verify real NativeSync transport. No fabricated endpoint, auto-resubmission, second execution authority or private archive publication.


## 2026-10-26 continuation candidate (not yet its own CI PASS)

Latest acf7e43 run40223285525: iOS SUCCESS (simulator 3/3), Ubuntu Rust bridge12/pending14/recovery6, Kotlin4, Android libraries/APK PASS; Android emulator completed 1/4 then stalled until job cancellation (overall CANCELLED). This revision adds atomic per-entry/Topic pending admission with seven new tests, and an 8-minute emulator bound + capped synthetic logcat on failure. Check this revision's actual CI before acceptance. Cargo.lock from acf7e43 artifacts was recovered and hash matched dc22b3703ebcffcfeb571842785832dab913609cb9fee0af37d658a67fac3a10, but not yet checked into this revision; --locked currently uses a generated lock. No production or real-device changes. M0 partial, M1 Android pending, M2 local component in progress, M3–M5 not complete.

## 2026-10-26 live run 40229145960 and next diagnostic revision

Commit `3dc56a4b0ca20ef46dd9c4cd40f39b98502469bd`: macOS/iOS simulator SUCCESS; Linux Rust 12 bridge+14 pending+6 recovery+7 Topic admission and Kotlin4 PASS; Android library/APK PASS. Emulator executed only 1/4 cases and globally timed out at 480s (exit124), so M1 Android FAIL/INCOMPLETE. This revision keeps all four original assertions but runs each in a fresh instrumentation process, with individual 100s timeouts and preserved per-method output. No assertion is deleted/relaxed. New portal preflight and 6 unit tests are source-grounded but NOT YET CI verified; still no real HTTP/Cookie transport. Cargo.lock remains to commit from the exact resolved dependency artifact. No deployment or production history changed.

## 2026-10-26 run40232585408 (verified)

HEAD `b6d0e24b580332906d07acd3a53125cd8e5076e4`: macOS/iOS simulator SUCCESS. Ubuntu Rust bridge12 + pending14 + recovery6 + Topic7 + portal6 (45 total), clippy, Kotlin4, Android native libraries and APK all PASS. Emulator method-isolated successes: success flow and structured error; `cancellationReachesNativeUi` did not finish within 100s (exit124). `recreatedActivityCanCallCore` was not run because the script stopped at cancellation. Android M1 therefore remains unaccepted. These are real GitHub Actions results, not inherited from previous SHA.

Next code revision tests a specific hypothesis (UniFFI suspend operation running on the Compose UI dispatcher): dispatch Rust `run` work on `Dispatchers.Default` while retaining Main-only state updates. It also records exact per-action Android test markers and runs lifecycle-recreation before the long cancel case so failure cannot mask that independent test. This is an unverified attempted fix pending its own CI; no assertion removed. Root cause of the earlier UI hang has NOT been proven. M0 partial, M2 local components passed only, M3–M5 not complete. NativeSync R2.1 source ZIPs are available as recorded in the source map; no accepted full checkout or native HTTPS transport is claimed.

## 2026-10-26: M1 verified at f678b1f (new flow pending CI)

Actions 40236267942 concluded SUCCESS for both Android/Linux and iOS/macOS jobs. The Android emulator ran all FOUR independent UI tests and each finished PASS: successAndCallbackReachNativeUi, structuredErrorReachesNativeUi, recreatedActivityCanCallCore, cancellationReachesNativeUi. iOS simulator UI 3/3 PASS. Rust, Kotlin/JVM, Swift/macOS host checks and target libraries and Android diagnostic APK also passed. This closes the **M1 build/simulator** criterion for the exact f678b1f commit only; no physical device, NativeSync service or signing acceptance.

The generated Cargo.lock from run40236267942 iOS artifact was recovered and SHA256-matched to dc22b3703ebcffcfeb571842785832dab913609cb9fee0af37d658a67fac3a10. It is NOT yet committed. This new revision changes no Cargo dependencies.

New M2 code: source-backed first-attempt request plans and local SQLite atomic dispatch claim, validated Original requestId/payload, exact-receipt observation and cross-session read-only recovery. It has NO HTTPS/Cookie implementation or model calls. The new code/tests require their OWN CI evidence before PASS; M2 remains incomplete.

## 2026-10-26 transport policy boundary (new CI pending)

Exact current HEAD bdf65c5 run40243107680 completed SUCCESS in both macOS/iOS and Linux/Android jobs, validating the prior M2 local first-attempt work. Added source-backed cookie issuance/revocation/scope handling, no-redirect HTTP status policy and authenticated session parsing; tests do not inherit that prior PASS. Real platform HTTPS/Cookie vault is NOT wired, M2–M5 remain incomplete. Only isolated branch source and synthetic tests change; no production or private ZIP publication.

## 2026-10-26 read-only HTTPS transport candidate, verification pending

Live HEAD d08c76e7ae6d4abb639ada5741b626ede6e80079 CI 40245151245: both Android/Linux and iOS/macOS jobs SUCCESS, validating the previous HTTP policy component. New isolated core/src/https_transport.rs uses a real pinned-origin, no-proxy, no-redirect Rust HTTPS session GET, in-memory scoped Cookie and bounded streaming response. NO real service connection, app invocation, protected platform vault, pairing, POST or real-device test occurred. This new code and reqwest dependency have no CI result yet; M2 still incomplete. Previous Cargo.lock hash cannot be reused after dependency update.

## 2026-10-26: next read-only history GET candidate (not yet verified)

Verified live commit 11e753e8a9ce4046bd21b9e4bf5835d0165c1142: run40250516435 both jobs SUCCESS, including actual Android 4/4 emulator and iOS 3/3 simulator, Swift/Kotlin host bridges and Rust source tests. That is not live service or device acceptance. Artifact native-ios ID12620309352 contained freshly resolved native/Cargo.lock SHA256 0b345c006c61a3ce179bb785f1306e180bdc2449bb2af2132be59742c0434465 (52,615 bytes); it is still not tracked, and this commit does not pretend otherwise.

This revision adds an authorized, Cookie-origin-scoped, read-only GET /mobile/api/conversations/:key?after=cursor, with page byte bounds, strict no-redirect/no-proxy origin, exact Topic/JS cursor checks and redacted history Debug. It leaves returned JSON opaque and tagged rather than prematurely materializing or claiming a verified receipt. The two new unit tests and this revision's full CI have **NOT YET PASSED**. No real server request has been performed. M2 remains incomplete, M3–M5 not accepted.

## 2026-10-26 latest verified revision and next POST candidate

Live HEAD `5412314cc55832717131b64aba9666d37c0997fe`, run40255755182: Ubuntu/Android and macOS/iOS jobs both SUCCESS; read-only Topic history GET and both platform simulator suites passed on this SHA. Not a real NativeSync service or device test. The iOS synthetic artifact 12622249545 included exact Cargo.lock (52615 bytes, SHA256 `0b345c006c61a3ce179bb785f1306e180bdc2449bb2af2132be59742c0434465`). Resolved lock remains untracked, no hand-created substitute.

This new revision implements a separately guarded, once-consumed POST transport for an original SQLite first-attempt claim, plus two synthetic request tests. It has not yet passed its own CI, and is not invoked from SwiftUI/Compose or a real server. M2–M5 remain unfinished.

## 2026-10-26: one-shot candidate CI repair and exact lock admission

At HEAD `e456769caee2b29bc4c4bd2b28b5233e5d57b9c7`, run `40259009246` concluded FAILURE on both hosts. Rust test groups (27+12+14+6+7) passed; `clippy -- -D warnings` rejected four ignored `#[must_use] OutboundSubmission` results in tests at `submission_flow.rs` 245/254/266/293. Platform steps were skipped. This revision explicitly drops the intentionally unsent first-attempt plans, preserving all assertions and strict linting. These tests verify durable local claims, not real HTTPS POST.

Native `Cargo.lock` linked from the verified earlier CI artifact Git blob `9d169cc99f0729d4b011f35835dc79816140fb68` (52,615 bytes; SHA256 `0b345c006c61a3ce179bb785f1306e180bdc2449bb2af2132be59742c0434465`). Dependency manifests unchanged from its generation. `check.sh` now fails without the lock instead of resolving dependencies silently, and Android/iOS Rust builds use `--locked`. No candidate vault files are included here; the uncommitted Keychain/Keystore ZIP remains available for a *selective* merge after CI, never wholesale overwrite of new POST.

**Status:** Code submitted for its own CI; success/failure must be read from that exact run, not inherited from earlier builds. M1 previous accepted simulator baseline still exists; M2 real server/device acceptance incomplete. No production, main, credentials or private ZIP changed.

## 2026-10-26: HTTPS Cookie debug redaction candidate (own CI pending)

Verified immediate predecessor ac4b9c5c1980e4e24c35c36354d8f0b0ec2266bf, Actions run40266175023: both Linux/Android and macOS/iOS jobs SUCCESS (no live server/device). Selectively carried forward the uncommitted vault candidate's sensitive Cookie-header treatment to current read-only GETs and extended it to the already implemented one-shot POST. The changes mark actual reqwest Cookie HeaderValues as sensitive (not merely redacting PortalCookie's own Debug); existing synthetic GET/POST tests now assert sensitivity and that POST request Debug does not expose the synthetic token. No authentication data or Cookie was stored, no vault host code was merged, and no production interaction occurred. This revision needs its own CI PASS; M2 server + mobile secure-vault acceptance remains incomplete.

## 2026-10-26: M2 secure-credential Rust bridge candidate (this revision needs own CI)
Read exact prior HEAD e866b2befba6fc9e72581e452873f41c5270ad1e; GitHub Actions 40268429452 on that HEAD completed SUCCESS (both platform jobs). Preserved the one-shot POST and sensitive Cookie-header changes. Selectively incorporated the previously uncommitted session-vault candidate's Rust module as new `core/src/session_credential.rs`, exported via UniFFI. It provides origin-scoped non-credential storage identifiers, bounded opaque records with original expiry (never TTL refresh), reject-only invalid/cross-origin/revoked/expired states, secret-redacted Debug, and caller-triggered read-only GET /mobile/api/session. Seven new Rust unit tests are included; they are NOT counted passed until this exact revision's CI completes. No Keychain/Keystore host implementation was merged, no credential was stored, and no live HTTPS or production service was touched. The original archive remains privately retained for further selective host integration; M2–M5 remain incomplete.
