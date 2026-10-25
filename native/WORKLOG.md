# Native implementation work log

## Authorized scope

Continue agreed R1.1 M0–M5 without asking at each milestone. Only development branch/native files and its workflow; no main/PR/merge/release, production/history/account/signing changes or paid models. Original Tauri untouched.

## First run 40216459025

Commit `64d66cad618c7338cd07c5ccbcf49f1f5b4641e6`: Rust 12 contracts, Swift-host and Apple-target library compilation passed. Kotlin compiled but JUnit initialization rejected the expression-bodied test with a non-Unit return. Android target was skipped, not passed. Next revision explicitly returned Unit from all test entry points, preserving all tests.

## App-host run 40221085134

Commit `7c2977f1105aa961922c1b3312a56e69eeb3faa0` added actual isolated Compose/SwiftUI hosts and UI automation. Kotlin/JVM 4/4 passed; Android libraries/APK compiled; Android emulator UI 3/4 passed. Cancellation timed out at final CANCELLED expectation. iOS arm64 26.5 simulator 3/3 passed, no skips/expected failures; not real iPhone evidence.

Retrieved Android artifact ID12609685925 SHA256 `abbfcb16c63b16f1eac970461ce115d39a3371676aec3761c336477b3e4a2787`; iOS ID12609419777 SHA256 `c09765f2a054b3d3b1cceec91917ef87bc4bea4a76f89e5264c625df9733013e`. Generated locks matched byte-for-byte for the prior dependency set. Original failure XML retained.

## This revision

The diagnostic used a fixed non-scrollable Column on a 320x640 emulator. Make it scrollable, scroll controls into view before physical clicks and show the synthetic semantic tree on expectation failure. Layout correction is implemented; causal sufficiency remains subject to rerun. No tests deleted or relaxed.

Added local pending-submission SQLite and 14 tests: original bytes/identity, namespace, exact cleanup, capacity, corruption, concurrency, cross-session query-only recovery and a child process exiting after commit. It sends no requests, stores no credentials by design and does not claim NativeSync R2.1 compatibility. Added explicit NDK C compiler/archiver target variables for SQLite compilation.

## Exact source dependency

ToolBox accepted commit `a9521bc32c1fb427d89ce6dcc0471c999202375f` still returned404. Project/Library lookup did not locate its archive. Old remote sources are reference-only. ProjectForge schema did not expose exact actions/epoch, so no guessed dispatch or RemoteDesktop bypass occurred. M0 source acquisition remains partial.

## Continued execution

The user extended implementation authorization through M5. Hourly external continuation checks live branch/CI and performs bounded available work. Waiting CI does not justify repeated cancellation or stopping the schedule. Only new real blockers or milestones should notify the owner. Real-device/signing/source requirements remain explicit; no background completion guarantee is inferred.
