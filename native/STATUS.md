# Implementation status

This revision: `M0_PARTIAL / M1_FIX_IN_PROGRESS / M2_LOCAL_COMPONENT_PENDING_TEST`.

## Scope

Continue M0–M5 on `codex/native-rust-m0-m1`, without separate approval at each code milestone. Not authorized: main/PR/merge/release, production or real-history changes, spending changes, credentials or device consent. Tauri and backend ownership remain intact.

## Observed evidence

- Initial `64d66cad618c7338cd07c5ccbcf49f1f5b4641e6`, run `40216459025`: 12 Rust tests and Swift-host/Apple-target checks passed. Kotlin JUnit initialization failed; Android target skipped.
- App hosts `7c2977f1105aa961922c1b3312a56e69eeb3faa0`, run `40221085134`: core and real host bindings passed; Kotlin XML 4/4. Android libraries/APK compiled, emulator UI 3/4 passed; cancellation final-state expectation timed out. iOS arm64 simulator 26.5 UI 3/3 passed, no skips; not real iPhone acceptance.
- Both retrieved Cargo.lock files matched exactly for that earlier dependency set. New SQLite dependencies need new lock resolution.
- This revision makes the Android diagnostic scrollable, scrolls controls into view before test clicks and emits a semantic tree on failure. It also adds bounded SQLite pending storage and 14 tests including child-process exit after commit. Its results require its own CI run, not inherited earlier passes.

## Source dependency

Accepted ToolBox commit `a9521bc32c1fb427d89ce6dcc0471c999202375f` remained unavailable through the current GitHub connection (404). Accessible Project/Library searches did not locate the source archive. VCPChat #68/#69/#70 are records, not a source worktree. Do not use the old reachable branch as accepted R2.1 or guess endpoints.

Local storage is not protocol adaptation, a second authority or a scheduler. M2/M3 remain unaccepted. Source retrieval does not prevent independent M1/platform work.

## CI and continuation

Standard public-repo Ubuntu/macOS runners only. No cache, production secrets, billing/account/visibility changes. Bounded synthetic XML/summaries/lockfiles and development APKs now use 1-day Actions artifacts, replacing the initial no-artifact setup.

Hourly continuation is arranged externally. Each run checks live HEAD/CI; in-progress CI or active concurrent work means defer that iteration, not cancel it or disable continuation. This is tool-dependent scheduled execution, not a guaranteed continuously running coding daemon. Notify only meaningful milestones/new blockers.

## Next

Read and fix the next exact CI result, preserve failures; retrieve/commit the new lock and use --locked. Confirm M1 only after both simulator paths pass for the claimed revision. Finish local storage verification and obtain real accepted NativeSync sources before compatibility acceptance. M3–M5 retain actual service, device and signing requirements; no fake success.
