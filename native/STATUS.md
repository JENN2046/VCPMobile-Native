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
