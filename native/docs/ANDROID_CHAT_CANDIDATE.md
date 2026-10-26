# Android chat candidate — local, not service/device acceptance

Integrated inputs: shared `286f819875856d2719580b1d576b7c1678d245e1`
(parent `072adcb`, receipt gate `60226d7`) and this thread's recovery/diagnostic
branch `e5e5b04aeac1a7ea3b847a9b03d69e006c04c5fc`. Shared run40346817148
finished both jobs SUCCESS. That is evidence for the shared input, not this
combined candidate. Jenn subsequently authorized one independent-branch push, one Issue #1
coordination comment and one Android build/emulator verification round. No
shared/main update, PR, merge, release or production operation is authorized. Existing owner's planned send UI overlaps this work;
coordinate the exact candidate diff before publishing either implementation.

## One store, claim and receipt gate

There is one exported `NativePendingJournal`, one existing first-attempt claim,
and one receipt-inspection implementation. Both earlier journal constructors
remain compatible and delegate to the same validated store owner. Platform
inventory, send and recovery select the same path. Either earlier on-disk layout
is retained; competing layouts fail closed for review. No automatic migration,
pending deletion, or replacement of unknown bytes is performed.

The shared gate's claimed-original requirement and second authenticated session
GET are preserved. It now produces the existing opaque recovery ticket; only a
reloaded unexpired credential under the vault lock/actor and matching current
view fence can consume it. The older Rust-only reconciliation method reuses
this same inspector. UI execution JSON is never used to authorize cleanup.

Android send performs an authenticated non-mutating preflight, then reloads
secure credential bytes and checks epoch plus entry/session/Topic/view fence
under the same vault lock used by save/remove. Only then does it persist exact
message bytes and consume the existing one-time first-attempt claim. The opaque
dispatch capability is single use. The transport still sends exact Origin,
Cookie and context guards without redirects/retries. All HTTP outcomes retain
pending, including 202; no AA-completion claim is inferred. Cancellation after
claim/HTTP initiation cannot prove the original was unsent.

The older `send_original_message_once` ABI is retained for compatibility, but
the mobile screen uses the fenced prepare/claim/dispatch path. The other owner's
future UI must not wire the older async seam as if it checked vault epochs.

## Android interaction and reply boundary

Pair or explicitly restore; select an authorized Topic; read history; enter a
message; explicitly send. The composer requires executable/history-ready Topic
metadata. On unknown results, the UI directs the user to the original receipt
action. Topic changes, local cancellation and credential replacement invalidate
late work. Startup never pairs, sends, or runs a model automatically.

Reply reception currently uses only the source-verified authorized history GET:
at most 120 reads, two seconds apart (100ms while draining existing pages),
400 in-memory rows. New central revisions replace the same message identity,
including tombstones. Stop reception cancels local reads, not the remote AA.
There is **no token-level SSE/WebSocket implementation or claim** in this batch.
The UI does not interpret arbitrary execution JSON as final AA authority.

Exact R2.1 installation/UX ZIPs were resolved in Library, but its normal transfer
helper failed with `hosted apps tools/list request failed: network`; no bytes
were materialized. One normal network-error retry also failed; no bypass was attempted. Normal GitHub access recovered
and verified public portal base blob `67bd37f...` at base `e84d51a...`; the
accepted patched portal `6950b91...` and accepted App.vue `c909ef6...` returned
404. The public base confirms the existing history/message routes but does not
establish the patched R2.1 live-token contract. Restore normal material access
or provide the exact expanded accepted source in this cloud workspace before
implementing an unverified event protocol. Original private ZIPs are not copied
into the repository.

## Actual local validation

- Rust **109** tests (70+12+14+6+7), zero failed/ignored; offline locked build.
- Strict clippy and release library build, actual UniFFI Kotlin/Swift generation.
- Android application and instrumentation-test Kotlin compilation.
- Kotlin actual-Rust host bridge: **6 passed**, zero failures/skips.
- Android JVM history-window tests: **3 passed**, zero failed/skipped.
- Synthetic TLS tests include the fenced new-send path, lost ACK, real SQLite
  reopen, exact original receipt, second-session drift and POST count exactly 1.
  Existing abrupt-subprocess recovery and no-retry cases remain.

The first Android unit run lacked two offline runtime JARs; a normal official
repository fetch resolved them and the same tests passed. This environment has
no Android NDK/Android Rust targets or Xcode; no complete device APK, new Android
emulator run or iOS app compilation is claimed. Generated Swift is not an Xcode
compile. Exact logs and the final commit belong to the local delivery record.

## Required real acceptance and next coordination

1. Publish only `codex/android-chat-candidate-20261026` and coordinate its exact
   commit in Issue #1 with the other active implementer. Its push triggers the
   existing workflow's Android job once; the iOS job is explicitly NOT RUN for
   that branch's push. The original shared-branch push still runs both jobs;
   manual dispatch defaults to both jobs, with an explicit `android_only` choice.
   All iOS steps/assertions remain unchanged. The Android job also runs the
   three view-state JVM tests before its existing emulator tests. Do not dispatch
   another run if the authorized push already created one. A green Android-only
   run is not dual-platform, actual service, phone or AA acceptance.
2. Supply the accurate R2.1 materials/isolated service version, a specifically
   approved HTTPS origin, synthetic authorized Topic/account and approved
   nonpaid execution arrangement. No production address, account, Cookie,
   credential creation or model invocation is implied by this code batch.
3. Supply an Android phone with authorized ADB/install access and a complete
   verified development APK. Follow `ANDROID_CHAT_DEVICE_ACCEPTANCE.md`.

iOS file protection remains **PENDING_REAL_DEVICE_ACCEPTANCE**, not passed.
Strict Complete, existing failing assertions and runs40319249884/40323326434/
40329845741 are retained. Wait for an iPhone and authorized signing/install
conditions to check root/main/live WAL/SHM and locked-device behavior. M2 real
service acceptance, actual token streaming, device use and M3–M5 are incomplete.
