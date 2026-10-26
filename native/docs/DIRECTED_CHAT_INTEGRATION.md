# Local directed chat integration, 2026-10-26

This is an unpublished local candidate. Fixed input is
`53e901732bec8a80d43e95720b5ab36f183d2c34`; selected UI source is
`d2b76e00eaada46c25ccdbbb509068c68215da24`. Their common ancestor
`286f819875856d2719580b1d576b7c1678d245e1` is not the current shared version.
This batch deliberately does not merge the shared branch. It keeps the
candidate's guarded sender, exact recovery, history revisions/tombstones and
one bounded Android history GET loop. Only confirmation/input policy and the
iOS composer are adapted from the shared UI. No workflow change or remote CI
belongs to this revision. Earlier green runs validate their own inputs only.

## One storage and send path

Both platform vaults have exactly one `pendingJournal(origin)` selector. It
calls `NativePendingJournal.openScoped(origin, trustedPrivateRoot)`, which
uses the same Rust resolver as the compatibility directory constructor.

| Existing layout within the trusted root | Handling |
| --- | --- |
| `vcp-pending-v1-<originKey>.sqlite` | Open in place |
| `<originKey>/pending.sqlite` | Open in place |
| `<serviceHash>/<originKey>/pending.sqlite` | Open in place, even after changing service/session |
| More than one layout, including an empty competing file | Reject and retain every file |
| Sidecar without main database, symlink or unrecognized database | Reject and retain, never initialize over it |
| No existing layout | Create only the canonical origin file |

Discovery scans at most 128 root entries and rejects larger inventories. This
is a conservative bounded failure, never a choice of an empty database. There
is no migration or UNKNOWN cleanup. Held journals recheck selection before
admission, dispatch and receipt consumption. The resolved path getter is for
the protected host to apply file attributes, not for UI or another opener.

The arbitrary-file legacy `NativePendingJournal.open` explicitly returns
`StorageRejected`. The old unguarded `send_original_message_once` explicitly
returns `GuardedSendRequired` without GET, claim or POST. Callers using those
ABIs must update; they are not fallback senders. The crate-private file opener
is used by the scoped constructor and existing low-level synthetic fixtures.
It is not exported through UniFFI.

The production call chain is:

1. Android confirmation -> `ConnectionViewModel.sendMessage` -> vault
   `sendMessage`; Swift confirmation -> `ConnectionModel.sendMessage` ->
   vault `sendOriginalMessageOnce` (a thin actor adapter).
2. Snapshot protected credential, epoch and the sole scoped journal; call
   Rust `prepareMessageSend` for a non-mutating authenticated session GET.
3. Reload credential under the same Android gate / Swift actor, check epoch
   and cancellation, then synchronously call `claimMessageSend`. Rust checks
   expiry, origin, credential fingerprint, entry/session, Topic and view fence.
4. The existing `first_attempt` algorithm admits original bytes and claims
   once in the same SQLite store. The guarded variant also checks its ephemeral
   preflight snapshot *inside* the existing IMMEDIATE admission transaction.
   SQLite `data_version` and own `total_changes` invalidate checks after either
   connection mutates the journal, including exact receipt cleanup. No schema,
   second claim mechanism or permanent replay database was added. Changes on
   another Topic may conservatively require a fresh deliberate preflight.
5. One opaque dispatch capability -> existing `submit_claimed_once` HTTPS
   adapter. All outcomes, 202 and cancellation after claim retain pending.
   No old check can recreate a confirmed original after cleanup.
6. Reopen/reconnect is read-only until explicit original receipt query.
   Current credential/epoch/view fence and original request identity are
   checked before consuming the opaque exact receipt and clearing that row.
   UI execution JSON has no cleanup authority; accepted/running is not AA
   completion. Cross-session recovery remains query-only.

Both composers use the Rust 4000 UTF-16-unit validation (including JS whitespace
semantics). Historic 12000-unit receipt/payload validation is unchanged, so an
old long original can still be reconciled. Confirmation is a UI affordance;
SQLite admission and protected capabilities provide the exclusion.

Android retains one loop, at most 120 existing history GET pages/updates, with
2-second idle waits and 100-ms backlog draining. This is **not token streaming**.
Swift retains explicit manual history reads and adds no polling loop. Stopping
the local wait never claims remote cancellation or certainly-unsent status.

## Actual isolated evidence

All tests below run on the original selected Linux cloud, without real Cookie,
NativeSync account, model, production history, physical phone or remote CI.

| Check | Result |
| --- | --- |
| Rust workspace/all-targets, pinned 1.90, locked/offline | 116 passed: 77 + 12 + 14 + 6 + 7; 0 failed/ignored |
| Strict clippy, workspace/all-targets, `-D warnings` | Passed |
| Release Linux library; actual Kotlin and Swift bindings | Generated; optional formatters absent |
| Kotlin JVM host tests against final Rust library, forced rerun | 8 passed; 0 failed/skipped |
| Android `compileDebugKotlin`, `compileDebugAndroidTestKotlin` | Passed, source compilation only |
| Android `testDebugUnitTest` | 3 passed; 0 failed/skipped (history revisions, deletion, bounds) |

New Rust regressions exercise all three prior layouts through new/compatible
constructors, cross-session inventory/reopen, competing empty stores, held
journal rejection, orphan WAL retention, UTF-16 boundaries and historic long
receipt identity. New TLS tests use real loopback TLS and real SQLite: same-ID
and different-ID concurrent claims on two instances; repeated dispatch;
202 and ACK loss; exact receipt only; dual layout before receipt consumption;
cancellation after the fixture accepted POST; login/Topic/cancel during an
in-flight preflight. POST counts are exactly 1 after a claim and 0 when the
preflight context is invalidated. Existing abrupt-subprocess-exit/reopen and
24 receipt-query failure scenarios remain active.

The delayed-check-after-exact-cleanup regression first failed against the local
intermediate candidate; the raw failure is retained in the local delivery
(`late-preflight-before-fix.log`, not a published-SHA or production failure).
It now checks both connections' held preflights and proves neither recreates
a cleared row. The two-connection tests assert a new reopened handle while
held checks exist; the separate existing subprocess test proves process reopen.
Core capability tests do not substitute for device-level vault/UI scheduling.

Reproduction commands from `native/` (use this environment's pinned toolchain):

```sh
export CARGO_HOME=/workspace/.vcp-toolchain/cargo
export RUSTUP_HOME=/workspace/.vcp-toolchain/rustup
export CARGO_TARGET_DIR=/workspace/.vcp-chat-target
export PATH=/workspace/.vcp-toolchain/cargo/bin:$PATH
cargo test --offline --locked --workspace --all-targets
cargo clippy --offline --locked --workspace --all-targets -- -D warnings
cargo build --offline --locked --release --lib -p vcp-mobile-native-core
```

Generate both bindings from the final release library using `uniffi-bindgen`
and `core/uniffi.toml`. Kotlin host tests need `target/release` to reference
that library (the local setup uses an ignored release-only symlink).

```sh
export JAVA_HOME=/workspace/.vcp-toolchain/jdk17-debian/usr/lib/jvm/java-17-openjdk-amd64
export GRADLE_USER_HOME=/workspace/.vcp-toolchain/gradle-home
export ANDROID_HOME=/workspace/.vcp-toolchain/android-sdk
export ANDROID_SDK_ROOT=$ANDROID_HOME
export ANDROID_USER_HOME=/workspace/.vcp-toolchain/android-user-home
/workspace/.vcp-toolchain/gradle/gradle-8.11.1/bin/gradle --offline \
  -Duser.home=/workspace/.vcp-toolchain/java-user-home -p tests/kotlin \
  --no-daemon test --rerun-tasks
/workspace/.vcp-toolchain/gradle/gradle-8.11.1/bin/gradle --offline \
  -Duser.home=/workspace/.vcp-toolchain/java-user-home -p apps/android \
  --no-daemon :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest
```

## Not run and next boundary

No Android NDK/Android Rust target, emulator/system image, Swift compiler or
Xcode is installed here. A complete new APK, Android instrumentation execution,
Swift/iOS compilation/host/UI tests and native platform vault race tests were
**not run**. Binding generation is not Swift compilation. Existing prior CI
is not inherited by this candidate. No artifact transfer was retried.

iOS strict Complete setter and original Complete assertion remain unchanged in
requirement; no assertion or test skip was weakened. Its storage test setup now
uses an explicit UUID-scoped trusted root and reads the resolver's selected
file. File protection remains **PENDING_REAL_DEVICE_ACCEPTANCE**, requiring an
iPhone and authorized signing conditions. Earlier simulator failures remain
failures, not acceptance. No further equivalent simulator protection run.

Parent should inspect this local commit/diff and call chain before choosing
whether to authorize publication/verification of this exact candidate. Only
a later expressly authorized round can establish Android build/emulator and
iOS compile compatibility. For real chat acceptance, still require a named
isolated NativeSync R2.1 endpoint, permitted synthetic account/Topic/history
and model policy, plus an Android device. No pairing, real credential creation
or real/paid model call is authorized by this local delivery. M2/M5 are not
complete; this is not a recommendation to merge the shared branch now.
