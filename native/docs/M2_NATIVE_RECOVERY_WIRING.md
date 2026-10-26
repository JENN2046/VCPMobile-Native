# Native original-receipt recovery wiring

## Exact integrated inputs

Shared history UI `238f450f4dbf31d69c2faeec7a2f0a6c900afb7f` and Rust receipt
query `be16729300a8782021c07ca1cefa56e4f3301baa` share base `dce262d`.
Their own runs40309677622 and40308456032 completed successfully on both
platforms. They are separate inputs, not verification of this combined batch.
Integration commit818a24d retains both parents and both WORKLOG additions;
its tree is the reviewed `d66244a98538f8fdde15c5e088c92501089e73d6`.

## Client flow and authority

After explicit session restore and Topic selection, both connection views
offer **核对待确认提交（只查询）** and cancellation. Launch, tab selection,
session restore and Topic selection never auto-run a recovery query or POST.
An empty journal shows `NO_PENDING_ORIGINAL`. This batch adds recovery, not a
send/composer or streaming UI. `checkpointOriginal` / `prepareOriginal` is a
storage-only entry for the explicit submission caller, preserving exact raw
JSON and Topic exclusivity before any future dispatch. It grants no network
authority and is not a mock message inserted into the UI.

`NativePendingJournal`, `NativePendingInfo`, `NativeRecoveryFence`, opaque
`NativeRecoveryCheck` and typed `NativeRecoveryOutcome` are exported through
UniFFI. Cookie bytes remain private to `SessionCredential`; no new Cookie
export, generic HTTP method, task control or retry entry is exposed.

1. Load the protected credential and its per-origin epoch; open the fixed
   app-private journal and select the original pending record for the Topic.
   Multiple ambiguous originals fail closed. Missing or expired credentials
   cannot reach HTTPS. Credential removal leaves the journal intact.
2. Create a view fence binding the **current** entry/session, Topic and view
   generation. The original request's session marker stays in its durable key.
3. `SessionCredential.queryPendingRecovery` checks expiry, origin, original key
   and fence, then performs the source-backed fresh authenticated session and
   original-Topic history GET. The actual current server context must match
   the captured current context. Validate the whole page and the exact receipt.
   Cross-session exact old receipts remain recognizable; absence is review-only.
4. After HTTPS, check expiry/fence again. Return an opaque token without pending
   mutation. The token is bound to this journal, this view fence and a private
   fingerprint of the original protected credential, including original expiry.
5. In the **same** Android vault gate / iOS actor, without a suspension gap,
   recheck epoch, reload the current credential and call synchronous confirmation.
   Rust rechecks expiry, credential fingerprint, entry/session/Topic/generation,
   single consumption and the entire stored original before transactional cleanup.
   All SQLite and protected storage work stays off the main UI thread.

View cancellation synchronously cancels the Rust fence **before** changing
generation or Topic. Confirmation and cancellation share its mutex. Once
cancellation returns, that fence cannot perform a later cleanup. A confirmation
already completed before cancellation is not rolled back. Closing the connection
view also cancels its recovery. iOS retains one vault actor per service.

No UI `executionsJson`, memory cache or model text is accepted as receipt
authority. Unknown, absent, rejected, expired, unauthorized, cancelled or late
results retain the original and never generate another POST/request identity.
`Accepted` only closes request-acceptance uncertainty; `running` stays running,
and the UI explicitly says it is not proof that AA completed. A cross-session
read grants no cancel/resolve/execute rights.

## Storage boundary

The journal is bounded by existing SQLite limits (16 records / 256KiB exact
payloads), FULL synchronous WAL, schema refusal and transactional Topic
admission. No Cookie is stored in it, and no cleanup is performed on expiry,
credential removal, corruption or an unrelated database.

- Android uses fixed canonical-origin directories under `noBackupFilesDir`,
  refuses device-protected storage and locked-user access, and relies on app
  isolation plus OS credential-encrypted storage. Keystore credential handling
  is unchanged.
- iOS uses Application Support with service/origin namespaces, backup exclusion,
  Complete file protection on the directory and SQLite/WAL/SHM files. Original
  device-only Keychain custody remains unchanged.

This is OS file protection, **not** application-level SQLCipher encryption.
SQLite DELETE does not guarantee physical erasure. No real account, history,
Cookie, production service or distribution signing identity is used in tests.

## Verification and remaining scope

Local Linux checks use the pinned Rust1.90, unchanged Cargo.lock, real
Swift/Kotlin binding generation, strict clippy and the actual Java17 compiler.
JDK17 was obtained from official Debian package indexes with SHA256 verification
after other official downloads returned403; no proxy bypass or system install.
Android application and instrumentation Kotlin sources compile locally.

Existing tests are retained. Rust now has96 tests (57+12+14+6+7). The existing
24 real-TLS synthetic ACK-loss scenarios are retained;17 native-bridge scenarios
cover current/original session separation, permission/context changes, expiry
before/after query, cancellation before/after query, cancellation/commit racing,
changed credential/journal, Topic/view-generation changes, missing evidence and
single consumption. An additional real subprocess exits without destructors
after the fixture accepts a POST but drops its ACK; its parent reopens SQLite/WAL
and confirms through the native bridge. Every such scenario asserts POST=1.

Kotlin/Swift host contracts each contain six checks, including actual pending
reopen, cancellation-before-network and unauthorized Topic refusal. Planned
current-platform counts are Android16 instrumentation methods and iOS15 simulator
tests: original secure storage/UI tests plus unpaired recovery, persistent
checkpoint/cancellation/removal and expired-credential retention. The iOS test
also checks file protection and backup exclusion. These platform test sources
need this batch's exact cloud run; prior passes are not inherited.

The TLS service is a synthetic protocol fixture, not actual NativeSync R2.1.
Rust process termination is not Android/iOS device-process acceptance. An
authenticated end-to-end mobile UI against the actual service, physical devices,
real streaming chat/OneRing/AA behavior and distribution signing remain untested.
No production trust settings or TLS checks are relaxed for fixture access.

The shared branch is not updated. Push only the independent approved validation
branch, read its exact SHA and run list first, and use at most one coordinated
existing dual-platform workflow. No workflow edit, duplicate run, PR, merge or
release is authorized by this batch. Remote run/commit outcome is recorded in
the delivery report; neither this document nor source completion means PASS.
