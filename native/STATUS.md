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
