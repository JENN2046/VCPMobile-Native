# VCPMobile Native client development

Independent SwiftUI / Jetpack Compose applications share a Rust client core through generated UniFFI bindings. Current screens are labelled **M1 diagnostics**, not a connected chat client. Existing Tauri/Vue code, server, main branch and history remain unchanged.

## Building blocks

- `core/src/lib.rs`: actual Rust structured errors, ordered callbacks, asynchronous wait, cancellation and single-run lifecycle. The original 12 tests remain.
- `core/src/pending.rs`: bounded SQLite original submissions, transactional persistence/cleanup and cross-session query-only decisions. Fourteen new contract tests; actual results tracked in STATUS.md.
- `apps/android`: Compose host, generated Kotlin binding, packaged Rust library and four emulator UI tests.
- `apps/ios`: SwiftUI/XcodeGen host, generated Swift binding and three simulator UI tests.
- `tests/kotlin` / `tests/swift`: actual host-to-Rust tests, not replacements for simulator or device tests.

NativeSync R2.1 accepted materials were recovered and partially blob-verified (see docs/NATIVESYNC_R21_SOURCE_MAP.md). A scoped HTTPS GET and one-shot POST implementation exists but has not been authenticated against the actual service; no full accepted checkout is claimed. Pending storage has no server acceptance or task execution authority.

## Build

Run `bash native/scripts/check.sh` with Rust/rustup on Linux/macOS. It uses `--locked` when a lockfile exists. CI retains generated lockfiles for verification; dependency additions without a resolved committed lock remain provisional.

- Kotlin: Java 17 + Gradle 8.11.1, `gradle -p native/tests/kotlin --no-daemon test`.
- Swift/macOS: `bash native/tests/swift/run.sh`.
- Android: `bash native/scripts/build-android.sh`, then `gradle -p native/apps/android --no-daemon :app:connectedDebugAndroidTest` with an isolated emulator.
- iOS: on macOS/Xcode/XcodeGen, `bash native/scripts/test-ios.sh`; actual simulator app, not signed iPhone distribution.

App/bundle ID `org.vcptoolbox.nativeclient.dev` is separate from the old app. The diagnostic requests no network/microphone permission and does not dispatch models/tasks. Callbacks are marshalled to the UI thread; storage work belongs off that thread.

## Evidence

Read STATUS.md and WORKLOG.md first. Source, host tests, target compilation, app compilation, simulator tests and device acceptance are distinct. An APK does not by itself establish M3 or M5. Preserve failed evidence, source identities and original repository LICENSE/attribution.

## Source-backed R2.1 first-attempt flow (not a network client)

- core/src/submission_flow.rs: bounded exact-origin GET plans; verified payload -> per-entry/Topic SQLite admission -> atomic first-attempt claim -> non-credential POST intent; subsequent attempts require receipt reconciliation, never auto-resend.
- docs/M2_SUBMISSION_FLOW.md: critical responsibilities for native platform Cookie/TLS handling and the actual HTTP/ACK integrations that are still missing.
- M1 evidence: run40236267942 on f678b1f is the first exact revision with BOTH Android emulator 4/4 and iOS simulator 3/3 passed. This does not establish M2–M5 or real-device acceptance.

## M2 read-only HTTPS candidate

core/src/https_transport.rs has scoped Rust HTTPS GET /mobile/api/session plus a one-shot POST transport, neither called from diagnostic UI. Android Keystore and iOS Keychain host source now exists, but real pairing, authenticated service testing and hardware acceptance remain absent. See docs/M2_HTTPS_READ_ONLY.md and docs/M2_SESSION_VAULT.md.


The next M2 read-only slice adds an authorized raw Topic history GET to `https_transport.rs`, without claiming actual server/device acceptance. The response is deliberately opaque; it cannot clear pending writes or add messages. See `docs/M2_HTTPS_READ_ONLY.md` and live STATUS for test evidence.

## M2 once-only HTTPS POST candidate

A new explicitly consumed and source-scoped `PortalHttpClient::submit_claimed_once` sends only a persisted first-attempt plan; its response never clears pending or grants task authority. The diagnostic apps do not invoke POST or hold a real Cookie. See `docs/M2_POST_TRANSPORT.md` and exact CI evidence.


**Reproducible dependencies:** `native/Cargo.lock` is committed from the exact successful CI-resolved dependency set; CI scripts fail if missing and build with `--locked`. Platform vault source has now been selectively added on top of current POST logic. Its new tests need this revision's exact CI before any PASS claim.
