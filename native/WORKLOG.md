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
