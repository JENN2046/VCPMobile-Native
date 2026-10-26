# Authenticated original-receipt query (local candidate)

Base: `dce262d03f36802e8c66f99953e9605f8fdeea14`. Work is isolated on
`codex/cloud-receipt-recovery-20261026`; no shared ref, Issue, PR or remote CI
was changed. The base's run40299726811 is completed/success. Its earlier
Keychain and loopback startup failures are resolved baseline history.

## Gap and implementation

The existing `reconcile_authenticated_history` accepted caller-provided JSON
and immediately cleared an exact pending record. It did not perform an
authenticated network read. This candidate adds
`PortalHttpClient::query_pending_receipt(cookie, store, original_key)`:

1. Load the original durable submission; missing/corrupt storage fails closed.
2. GET the actual current authenticated session with the existing pinned HTTPS
   client and scoped Cookie. Reject changed entry or revoked original Topic.
3. GET the **original** Topic's history with `after=0`, independently of the
   current selection. Validate the complete typed history page before receipt
   inspection. Executions are unpaginated, so additional message pages are not
   requested merely to find the receipt.
4. Reuse the existing exact request/kind/payload hash/original session oracle.
   A different current session may recognize the exact old receipt; absence
   across sessions stays review-only. Missing, conflicting, ambiguous or
   malformed evidence never authorizes a resend.
5. Return an opaque, non-Clone `PendingReceiptCheck`. Querying never mutates
   pending. The platform must check credential expiry and credential/view
   epochs, then either discard the token or consume `commit(store)`.
   Only an accepted exact receipt permits SQLite cleanup, which compares the
   complete original again inside its transaction. A replaced row is retained.

There is no POST, retry, new request ID, task cancel/resolve, model or AA call
in this API. An accepted receipt whose execution state is `running` remains
`running`; this is request acceptance, not successful AA completion.

The production HTTPS builder retains its exact TLS/hostname checks, HTTPS-only,
no-proxy, no-redirect and no-retry settings. Test-only CA/loopback injection is
behind `cfg(test)` and reuses that builder. No dependency or lockfile changes.

## Exact local validation

Linux cloud environment, Rust/Cargo 1.90.0 matching `native/rust-toolchain.toml`:

```
bash native/scripts/check.sh
cargo test --locked --workspace --all-targets  # from native/
cargo clippy --locked --workspace --all-targets -- -D warnings
```

The full script passed: 55 library + 12 bridge + 14 pending + 6 recovery + 7
admission = **94 tests**, zero failed/ignored, strict clippy, release library,
and actual Kotlin/Swift binding generation. Optional generated-source
formatters `ktlint` and `swift-format` are absent; generation itself succeeded.
After the final test addition/formatting, the same 94 tests and strict clippy
were run again successfully.

Two new Rust tests include **24 synthetic real-TLS scenarios**. In each main
scenario, an isolated Python fixture accepts one synthetic guarded POST and
closes the connection before any ACK. The test reopens the actual SQLite file,
checks the exact original bytes and pending state, and proves another first
attempt is refused. Recovery then performs only authenticated GETs. Final
request counters assert POST=1, no redirects followed, no task routes, and no
automatic history pagination. Repeated refresh still leaves POST=1.

Scenarios cover exact and cross-session receipt, same/cross-session absence,
conflict, duplicate, missing metadata, wrong Topic, malformed whole page,
changed entry, revoked Topic, session/history 401 and 403, session/history
redirect, unexpected Cookie rotation, dropped history response, wrong MIME,
paginated messages with an unpaginated receipt, repeated reads, discarded late
result and a changed local original before commit. Missing original storage
also fails before networking.

The receipt payload hash is independently computed in Python from the
documented canonical fields. TLS uses ephemeral test CA/server certificates
with hostname verification enabled. Only request counters are written by the
fixture. No actual Cookie, production history or model service is accessed.

## Remaining work and coordination

This candidate is a Rust transport/recovery increment, **not** complete native
chat or actual NativeSync acceptance. It deliberately leaves the known
history UI/cache work to the other owner. There is no new UniFFI journal API,
Android send/history/stream UI, platform expiry/epoch wrapper or live-service
connection here. The caller still owns protected Cookie custody and must
check expiry and epochs before using/committing a query.

Database close/reopen is tested, not Android process killing or physical power
loss. The service is a clearly synthetic protocol fixture, not the exact R2.1
server. Pairing, real streaming/results, actual OneRing/AA behavior, real
service recovery, Android/iOS app builds, simulators and devices were not run
for this candidate. Swift/Kotlin generation does not substitute for compilation.
The optional local JVM host test was not run: JDK17 download from Adoptium
returned HTTP403; this does not block the completed Rust tests. No cloud CI
was triggered to work around it.

Before integration, coordinate the exact diff with the foreground/hourly
owner and re-read remote HEAD. Rebase or cherry-pick without replacing newer
files, expose the recovery token through the existing vault epoch checks,
then coordinate one shared CI run. Shared-branch push/Issue/PR/merge/release
requires the parent thread's further confirmation.
