# Local R13 directed integration, 2026-10-27

Unpublished Windows working-tree candidate based on
`ee9a666af40063d060c251b30d3b6ee1f9c48614`. No push, PR, remote CI,
production connection, old-queue replay, or physical-device acceptance.
This supersedes the earlier R9-only intake scope, not either fixed Git input.

## Exact inputs and replay

| Input | Identity |
| --- | --- |
| Our verification branch | `codex/native-directed-verification-20261026`, `ee9a666af40063d060c251b30d3b6ee1f9c48614` |
| Shared baseline | `codex/native-rust-m0-m1`, `d2b76e00eaada46c25ccdbbb509068c68215da24` |
| Shared tree | `e6d89e88ed3d7d4c11bf791c722b3cebd56953da` |
| R13 ZIP | `VCPMobile_Native_M2_R13_iOS_Pending_Review_Reauth_Local_20261027.zip`, 100520 bytes |
| Observed ZIP SHA256 | `4cc9ec64eca168f29e0ae68a82ae388b022c514c34507dd78b117843e3953747` |
| Cumulative patch SHA256 | `4e7dda8c55fc51aae7403cf8c71b2581cc01ea321e23b94dd8875aa6648cb653` |
| R12 to R13 patch SHA256 | `5146a86affd6fe2fe71053bb060fe9d565c25fc2614d6952e898f1fd46bb6611` |

The original conversation's latest completed delivery observed during intake was
R13. Its subsequent frozen-handoff request was still awaiting a completed reply;
the user then supplied the R13 ZIP's local path. The ZIP digest above is computed
from those supplied bytes, not a previously user-supplied expected digest.
Both patch digests match the packet's manifest/checkpoint.

Both branch tips were checked read-only through GitHub and `git ls-remote` and
matched the fixed commits. `P:\VCPMobile-Native` initially was an empty directory,
not a repository. A new clone was detached at our exact commit; no pre-existing
changes were overwritten or stashed. R9/R13 reference replays are separate
detached worktrees under sibling evidence directories, never merge inputs.

The seven ZIP members passed traversal, case-collision, special-file, size and
CRC checks before extraction. The full patch applied to the exact shared
baseline in a reference worktree. Windows checkout CRLF was normalized to LF
only in that reference replay, then **118 source files matched raw SHA256**.
The manifest's 119th item, `SOURCE_MANIFEST.json`, is absent from the supplied ZIP
and patch. It is not recreated or counted as verified. No production source
listed in the R13 manifest was missing. R11/R13 inclusion was confirmed in the
replayed methods, not inferred from filenames or delivery prose.

## Selection and adaptation

| Area | Decision |
| --- | --- |
| Rust core, journal resolver, transactional admission, receipts, Cargo.lock | Retain our input unchanged; do not import the cumulative packet's older core |
| Both protected Vault implementations and Vault tests | Retain our input unchanged |
| R9 | Import strict revision/tombstone merge, iOS explicit bounded history reception, corresponding tests |
| R11/R10 | Import foreground suspension, synchronous generation invalidation, owned history/inventory task cancellation, late-result/finalizer guards, UI lifecycle hooks |
| R13 | Revoke old pending clearance synchronously before inventory/recovery; only current authenticated empty inventory grants UI readiness |
| R12 | Import bounded execution-state labels; presentation only, no receipt authority |
| Confirmation/topic guards | Import and retain our Rust UTF-16 validation; consume the view generation before sending so later rechecks cannot revive a captured confirmation |
| Android stop | Explicitly invalidate the old generation and clear its chat handle before a new poll; revoke history readiness |
| iOS send | Keep the existing protected `sendOriginalMessageOnce` adapter and send cancellation UI; permit topic switches to cancel active send/reception |
| Test gates | Adapt send-path checks to our common Rust selector and existing iOS adapter name; add discovery, reauth/lifecycle mutation checks and XML verifier |
| Workflow | Unchanged; no trigger, branch, or remote evidence changes |

The Android sender is still `ConnectionViewModel` -> `PortalSessionVault.sendMessage`.
The iOS sender is still `ConnectionModel` ->
`PortalSessionVault.sendOriginalMessageOnce`. R13's alternative Vault sender was
not introduced. Recovering an accepted receipt does not itself grant new UI send
readiness; an explicit current empty-inventory check is required.

## Call-chain audit

1. Both platform inventory/send/recovery paths use their one `pendingJournal`
   method -> `NativePendingJournal.openScoped` -> Rust `journal_location::resolve`.
   Existing supported stores are opened in place. Competing stores, including an
   empty rival, are rejected and retained; there is no automatic empty-store choice.
2. Vault snapshots protected credential/epoch and calls `prepareMessageSend`.
   After the await, Android rechecks/reloads and calls `claimMessageSend` within
   `synchronized(gate)`; iOS performs the corresponding sequence without an await
   inside the Vault actor. There is no new database or claim implementation.
3. Rust verifies origin, credential fingerprint, entry/session/topic, view fence,
   cancellation and store change stamp in the existing admission path. An
   IMMEDIATE transaction excludes competing original sends for the same topic.
4. One opaque dispatch capability feeds the existing single POST path. 202,
   dropped ACK and cancellation after claim retain the original pending record.
   The legacy unguarded sender returns `GuardedSendRequired`.
5. Recovery performs authenticated exact-original receipt queries and protected
   epoch/reload/consume. Only the unique exact receipt can clear that row. UI
   history/execution JSON, empty rendering, errors and UNKNOWN cannot clear it.
6. History is explicit bounded GET polling, at most 120 reads, with generation,
   context and (iOS) poll-token checks. Stopping local reception does not cancel
   remote execution. Backgrounding invalidates before cancelling read handles;
   old callbacks/finalizers cannot grant readiness or release a newer read's busy state.

## Evidence and limits

Evidence is stored separately at `P:\VCPMobile-Native-R13-20261027`. The delivery
manifest binds the final source hashes, generated binding/library hashes, patch
hash and test logs. Counts below are actual local results, not inherited CI.

| Check | Local result |
| --- | --- |
| Rust 1.90.0 `cargo test --locked --workspace --all-targets` | 115 passed: 76 + 12 + 14 + 6 + 7; 0 failed/ignored |
| Strict Clippy `--workspace --all-targets -- -D warnings` | Passed |
| Rust debug Windows DLL and actual Kotlin/Swift UniFFI generation | Passed; optional ktlint/swift-format absent; binding generation is not Swift compilation |
| Kotlin JVM -> actual Rust DLL bridge | 8 passed, 0 failed/skipped |
| Controlled-I/O JVM tests of extracted production methods | 7 passed, 0 failed/skipped |
| Android `compileDebugKotlin`, `compileDebugAndroidTestKotlin` | Passed against actual generated bindings; source compilation only |
| Android `testDebugUnitTest` and XML completeness verifier | 10/10 passed; 0 failed/skipped; report identities matched all four source files |
| Single send/journal source tripwire | 20/20, source-only |
| R11 lifecycle source gate | Passed; 4 negative mutations rejected |
| R13 pending-reauth source gate | Passed; 4 negative mutations rejected |
| Android instrumentation discovery | 19/19 methods configured; not executed on a device |
| Android JVM report verifier self-tests | 15 passed; synthetic XML, not app test evidence |

The Rust TLS fixtures use real local TLS and SQLite, with synthetic cookies and
service replies. They exercise same-topic admission on independent connections,
single POST, 202, dropped ACK, exact/ambiguous receipts, cancellation, changing
login/topic during preflight, and abrupt child-process reopen. On Windows one
Unix-specific filesystem test is not compiled; hence 115 rather than an old
116-test Linux claim. No ignored test is counted as passed.

The seven extra JVM tests compile the actual current production method bodies,
use actual generated record types, and control non-cancellable vault continuations.
They cover failed reauthentication after a prior grant, current empty/nonempty
inventory, background late inventory/history versus a newer read, session/topic
change, stop invalidation and repeated confirmation. The Vault, lifecycle host
and send counter are substitutes. They do not demonstrate Android Main dispatcher,
Keystore, Compose lifecycle delivery, JSON parsing, or physical-device behavior.
`prepare_android_read_fence_host.py` records the extracted source hashes.

Initial failures are retained: offline Cargo lacked cached reqwest; TLS tests
could not find OpenSSL, then resolved Windows' nonfunctional `python3` alias;
the first JVM bridge run used the project's default library directory and could
not load the DLL. Process-local PATH/PYTHONHOME and the test task's DLL directory
fixed these environment failures without modifying the core or weakening tests.

Not run: complete Android NDK APK build, emulator/instrumentation execution,
Swift host tests, Xcode/SwiftUI compilation, iOS simulator, real NativeSync,
physical Android/iPhone, authorized device signing and lock/unlock acceptance.
The known `ee9a666` iOS strict file-protection failure is not converted to success.
`FileProtectionType.complete` setters and the original assertions are retained.
Status remains **PENDING_REAL_DEVICE_ACCEPTANCE**. No dual-platform-green claim.

## Local reproduction

Run the Python source gates in `native/scripts`, then the locked Rust workspace
tests and Clippy using Rust 1.90.0. Windows TLS fixtures require a working
`openssl` and `python3` on the child process PATH. Use the generated bindings
from that same library. The sibling `windows-test-library.gradle` points JVM
tests to the actual debug DLL and includes the generated controlled-I/O tests;
it does not change repository configuration. Gradle is 8.11.1 and JDK is 17.

For Android source verification, run `:app:compileDebugKotlin`,
`:app:compileDebugAndroidTestKotlin` and `:app:testDebugUnitTest`, then
`verify_android_jvm_tests.py`. The shell build scripts also wire the verifier
for a later authorized full platform build; adding checks does not run CI.
