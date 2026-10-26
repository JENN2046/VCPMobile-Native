# VCPMobile Native client development

SwiftUI (iOS) and Jetpack Compose (Android) share a Rust client core through UniFFI. The app has an M1 diagnostic screen and an experimental M2 connection screen. It is not yet a complete chat client.

## Current capabilities

- Shared asynchronous bridge, structured errors, progress and cancellation.
- Bounded SQLite original submissions, source-backed request identity and receipt reconciliation; unknown operations are never silently replayed.
- Scoped HTTPS session/history reads, typed validated history pages and consumed one-shot submission plans. History reads recheck authenticated context and platform credential epochs. The experimental read-only Topic history view remains available alongside the new recovery controls; it is not a complete chat UI.
- Explicit one-use pairing, Keychain/Keystore credential storage, session restore and authorized Topic keys. Save precedes session GET; a failed GET does not cause another pairing POST.
- Connection controls for local credential removal, clearly separate from remote logout or task cancellation. No request runs on screen launch; no microphone permission or model dispatch is added.
- Explicit original-pending recovery through the protected vault: fresh authenticated reads, revocable view fences, credential/epoch recheck and exact single-use receipt consumption. Pending survives restart and local credential removal. See `docs/M2_NATIVE_RECOVERY_WIRING.md`; this is recovery wiring, not a new send/streaming UI.

Only the non-secret last origin is stored in ordinary settings. Pairing codes are ephemeral, and credentials remain in platform protected storage. Original Tauri client and service-side History/OneRing/AA authority are unchanged.

## Build

Run `bash native/scripts/check.sh` on Linux/macOS with Rust/rustup. A committed `native/Cargo.lock` is required and Rust paths use `--locked`; never silently resolve a replacement lock. Host TLS contract tests also use Python3 and OpenSSL with ephemeral loopback test keys.

- Kotlin: Java17 and Gradle8.11.1; `gradle -p native/tests/kotlin --no-daemon test`.
- Swift host: `bash native/tests/swift/run.sh`.
- Android: `bash native/scripts/build-android.sh`, then `bash native/scripts/test-android-emulator.sh` on an isolated emulator. Instrumentation runs four original UI, ten vault/read/recovery guards and two connection tests individually.
- iOS: macOS/Xcode/XcodeGen, `bash native/scripts/test-ios.sh`. Simulator-local signing needs no developer account or distribution secrets. Original Keychain/UI assertions are retained; history and recovery guards bring the current suite to15 tests.

App ID `org.vcptoolbox.nativeclient.dev` is separate from the old client. Android storage work runs off Main; platform UI receives shared-core state. iOS cloud simulator tests do not establish iPhone or signed-distribution acceptance.

## Evidence and source

Read `STATUS.md`, `WORKLOG.md`, Issue #1 and the latest exact Actions result. History UI input238f450/run40309677622 and receipt inputbe16729/run40308456032 each passed their own dual-platform checks. The combined recovery-wiring revision requires its own exact run; neither input's PASS substitutes for it.

NativeSync accepted source/patch identities and verified scope are in `docs/NATIVESYNC_R21_SOURCE_MAP.md`. The source ZIPs are available but private and are not published here; partially verified files are not described as a full checkout. `docs/M2_PAIRING.md` explains the actual pairing contract and remaining service acceptance.

The new TLS fixture uses real transport but synthetic protocol responses. Actual NativeSync service pairing, authorized history, message/AA continuation and ACK-loss acceptance remain separate work. Source completion, compilation, simulator, service integration, physical devices and signing are reported separately. Preserve original LICENSE/attribution and failed evidence.

## Explicit local pending inventory (candidate)

Both native Topic screens now offer an explicit, read-only inventory of
per-origin application-private pending request identities after a fresh
authorized session GET. It does not submit, retry, confirm receipts,
or restore old AA authority. The submission UI, exact receipt closure and
real NativeSync isolated-service acceptance remain next tasks. Evidence and
privacy boundaries: `docs/M2_PENDING_JOURNAL.md`. This candidate must pass
its own CI before claiming a platform build result.
