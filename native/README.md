# VCPMobile Native client development

SwiftUI (iOS) and Jetpack Compose (Android) share a Rust client core through UniFFI. The app has an M1 diagnostic screen and an experimental M2 connection screen. It is not yet a complete chat client.

## Current capabilities

- Shared asynchronous bridge, structured errors, progress and cancellation.
- Bounded SQLite original submissions, source-backed request identity and receipt reconciliation; unknown operations are never silently replayed.
- Scoped HTTPS session/history reads and consumed one-shot submission plans. Native message/history UI is not yet connected.
- Explicit one-use pairing, Keychain/Keystore credential storage, session restore and authorized Topic keys. Save precedes session GET; a failed GET does not cause another pairing POST.
- Connection controls for local credential removal, clearly separate from remote logout or task cancellation. No request runs on screen launch; no microphone permission or model dispatch is added.

Only the non-secret last origin is stored in ordinary settings. Pairing codes are ephemeral, and credentials remain in platform protected storage. Original Tauri client and service-side History/OneRing/AA authority are unchanged.

## Build

Run `bash native/scripts/check.sh` on Linux/macOS with Rust/rustup. A committed `native/Cargo.lock` is required and Rust paths use `--locked`; never silently resolve a replacement lock. Host TLS contract tests also use Python3 and OpenSSL with ephemeral loopback test keys.

- Kotlin: Java17 and Gradle8.11.1; `gradle -p native/tests/kotlin --no-daemon test`.
- Swift host: `bash native/tests/swift/run.sh`.
- Android: `bash native/scripts/build-android.sh`, then `bash native/scripts/test-android-emulator.sh` on an isolated emulator. Instrumentation runs all four original UI, six vault and two new connection tests individually.
- iOS: macOS/Xcode/XcodeGen, `bash native/scripts/test-ios.sh`. Simulator-local signing needs no developer account or distribution secrets. The app-hosted six Keychain tests and original three UI tests are retained; two connection UI tests are added.

App ID `org.vcptoolbox.nativeclient.dev` is separate from the old client. Android storage work runs off Main; platform UI receives shared-core state. iOS cloud simulator tests do not establish iPhone or signed-distribution acceptance.

## Evidence and source

Read `STATUS.md`, `WORKLOG.md`, Issue #1 and the latest exact Actions result. `6c6b4c4` / run40290030154 passed both platform jobs, including iOS9/9 with no skips. New pairing code requires its own CI, not inherited PASS.

NativeSync accepted source/patch identities and verified scope are in `docs/NATIVESYNC_R21_SOURCE_MAP.md`. The source ZIPs are available but private and are not published here; partially verified files are not described as a full checkout. `docs/M2_PAIRING.md` explains the actual pairing contract and remaining service acceptance.

The new TLS fixture uses real transport but synthetic protocol responses. Actual NativeSync service pairing, authorized history, message/AA continuation and ACK-loss acceptance remain separate work. Source completion, compilation, simulator, service integration, physical devices and signing are reported separately. Preserve original LICENSE/attribution and failed evidence.
