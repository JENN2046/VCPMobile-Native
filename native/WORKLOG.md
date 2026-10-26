# Native implementation work log

## Authorization

Continue R1.1 M0–M5 without per-stage approval. Only development branch/native files and its workflow. No main/PR/merge/release, production/history/account/signing changes or paid model calls. Original Tauri and VCP backend remain unchanged.

## Preserved CI history

64d66cad618c7338cd07c5ccbcf49f1f5b4641e6 /40216459025: Rust12, Swift host and Apple target libraries passed. Kotlin initialization failed from non-Unit test return; Android skipped. Next revision explicitly returned Unit, no tests deleted.

7c2977f1105aa961922c1b3312a56e69eeb3faa0 /40221085134: Kotlin4/4, Android APK compiled, emulator3/4 with cancel final-state timeout; iOS simulator3/3 passed. Artifacts Android12609685925 SHA256 abbfcb16c63b16f1eac970461ce115d39a3371676aec3761c336477b3e4a2787; iOS12609419777 SHA256 c09765f2a054b3d3b1cceec91917ef87bc4bea4a76f89e5264c625df9733013e. Locks matched for that dependency set.

8f907587e4d952efc63df4b5e25fa18e51659af6 /40222103042: scrollable Android controls plus stronger failure diagnostics; SQLite pending storage with14 tests including real child-process exit after commit. Linux core/host/build passed at observation; emulator still running. macOS script failed at empty `lock_args[@]` under old Bash nounset before compilation, not at a Rust test. This revision removes that array rather than hiding the failure.

## Accepted source material recovered

Earlier commit/tree404 and content-search misses remain historical facts. Searching Library ZIP metadata found NativeSync_R2.1_Install_20261024.zip and UX3 ZIP. Installation archive size/digest/CRC matched original delivery. Cumulative patches matched their published digests. 26 new-file hunks and2 additional exact baseline files yielded28 accepted blob-verified ToolBox files. No complete checkout/tree rebuild is claimed. Original archives were not copied wholesale into the public repository.

Ran exact accepted client submissionRecovery.js under Node22.16.0 to generate50 synthetic hash/receipt cases. New Rust port uses explicit UTF-16 limits, ECMAScript whitespace, normalized attachment hashing and strict original receipt matching. It does not send requests or confer task control. Storage/transport/topic admission integration remains for M2.

## Build and evidence repair

Core script always uses --locked after generating a missing lock. Local synthetic shell tests checked existing/missing lock and propagated test failure. Tracked-source packaging excludes untracked data; optional resolved lock is marked separately; modified tracked lock is refused. Source archive/hash/manifest and bounded synthetic core logs join1day CI artifacts. This is not real local Rust compilation or M5 completion.

Hourly continuation is active; it reads live records and advances bounded safe code work. Source lookup is no longer an all-work blocker. Device/signing requirements stay explicit; no guaranteed unattended daemon is implied.


## 2026-10-26 live run and M2 admission

Read acf7e4300f5c457867b2279de7d26d9373565a57 run40223285525: iOS simulator 3/3 PASS; Rust bridge12/pending14/recovery6 PASS; Kotlin4/4 and Android APK PASS; Android UI 1/4 only before 35-minute cancellation, so the overall run is CANCELLED. Cargo.lock artifact SHA256 dc22b3703ebcffcfeb571842785832dab913609cb9fee0af37d658a67fac3a10 matches resolved-dependencies/Cargo.lock; lock commit remains an explicit next action. This batch adds transactional per-entry/Topic admission tests, preserving unknown across sessions and kinds. No network authentication or model calls. Eight-minute emulator diagnostic bound does not delete/relax tests. Read next CI and fix its actual outcome.

## 2026-10-26 live run 40229145960 and next diagnostic revision

Commit `3dc56a4b0ca20ef46dd9c4cd40f39b98502469bd`: macOS/iOS simulator SUCCESS; Linux Rust 12 bridge+14 pending+6 recovery+7 Topic admission and Kotlin4 PASS; Android library/APK PASS. Emulator executed only 1/4 cases and globally timed out at 480s (exit124), so M1 Android FAIL/INCOMPLETE. This revision keeps all four original assertions but runs each in a fresh instrumentation process, with individual 100s timeouts and preserved per-method output. No assertion is deleted/relaxed. New portal preflight and 6 unit tests are source-grounded but NOT YET CI verified; still no real HTTP/Cookie transport. Cargo.lock remains to commit from the exact resolved dependency artifact. No deployment or production history changed.

## 2026-10-26 run40232585408 and dispatcher diagnostic fix

Android artifact ID 12614021192 confirmed that success and structured-error tests each ran as standalone JUnit tests and passed. The cancellation test emitted START instrumentation but no finish and reached its 100s hard timeout; filtered logcat contained test-start but no inner-step checkpoints. iOS job succeeded. The Rust suite tested 45 cases (12 bridge,14 pending,6 recovery,7 topic,6 portal), Kotlin4 passed and Android APK compiled. The correct conclusion is Android M1 incomplete, not a platform build failure.

This revision explicitly moves the asynchronous UniFFI invocation to Dispatchers.Default and returns UI state updates to the original Main coroutine; adds action-level Logcat markers without removing any UI assertion; keeps four independent instrumentation cases, executing the lifecycle case before the long cancellation case. This is a hypothesis-driven repair, not a claimed root-cause proof. If cancel remains stuck, use the new last-checkpoint evidence to locate the blocked action rather than further masking the test. No real VCP service was accessed.

## 2026-10-26: M1 cloud simulator passing; M2 first-attempt claim added

Re-read live HEAD f678b1f137532ced950655f2eeb633e568b71b9c and run40236267942. Android 4/4 named emulator instrumentation PASS and iOS simulator 3/3 PASS; both CI jobs SUCCESS. Earlier canceled/timed-out runs remain as original evidence. This is NOT Android/iPhone physical-device testing.

Added pure Rust submission_flow with exact HTTPS origin + authorized GET plans, source-derived payload and context guard, one initial outbound POST intent only after a persisted SQLite claim. Conditional UPDATE atomically fences racing connections and survives process reopen. A failed/ambiguous server receipt never generates an automatic second submission. Reconciliation accepts only exact verified request fingerprint and authorized Topic context, including cross-session READ confirmation without task control. Synthetic tests added but require new CI.

Actual native HTTPS transport, protected Cookie lifecycle, response provenance/status, real service and physical-device acceptance remain pending. No private ZIP copied to the public repository. Generated Cargo.lock from CI has matching recorded hash but is not tracked yet.

## 2026-10-26 transport policy boundary (new CI pending)

Exact current HEAD bdf65c5 run40243107680 completed SUCCESS in both macOS/iOS and Linux/Android jobs, validating the prior M2 local first-attempt work. Added source-backed cookie issuance/revocation/scope handling, no-redirect HTTP status policy and authenticated session parsing; tests do not inherit that prior PASS. Real platform HTTPS/Cookie vault is NOT wired, M2–M5 remain incomplete. Only isolated branch source and synthetic tests change; no production or private ZIP publication.

## 2026-10-26 new HTTPS GET slice

Observed live HEAD d08c76e7ae6d4abb639ada5741b626ede6e80079 and CI run 40245151245 both SUCCESS. Implemented a real caller-triggered GET /mobile/api/session in Rust, verified source-defined path and strict TLS/Cookie origin, response size, no redirect or automatic cookie jar. No network to production, no task authority or side effects. This change must be tested in its own CI, and a newly resolved Cargo.lock must be committed next. No host Keychain/Keystore yet.

## 2026-10-26: verified GET session build and history GET candidate

Live HEAD 11e753e8a9ce4046bd21b9e4bf5835d0165c1142, run40250516435 concluded SUCCESS in both Linux/Android and macOS/iOS CI jobs; initial read-only session GET client tests and all M1 simulators passed on that SHA. Downloaded synthetic iOS artifact12620309352, verified Cargo.lock exact SHA256 0b345c006c61a3ce179bb785f1306e180bdc2449bb2af2132be59742c0434465; resolved lock remains untracked. The next slice adds a real but uninvoked authorized history GET builder and body reader with strict context, no redirects, no cookie changes and no auto-application. All real message schema interpretation, app vault wiring and authenticated service fixtures remain pending. This new revision needs new CI evidence.
