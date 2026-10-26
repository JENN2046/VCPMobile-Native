# M2 pairing and native connection entry

This change implements a user-triggered pairing path; it is not a claim that the actual NativeSync service or a physical phone has passed acceptance.

## Verified predecessor

Exact commit `6c6b4c4700e2f06358431dd0fe7f90cad6835619`, Actions `40290030154`: both platform jobs SUCCESS. iOS summary has 9 passed, 0 failed/skipped/expected failures (six real Keychain + three original UI cases). Artifact `12635887933` SHA256 `b32b5fb5be7d4a3e91cb7237ccefa19bdbb11cf4c5f06b900e24ece9997ecc80` was downloaded, CRC checked and all 60 scoped source hashes verified. This closes the specific simulator Keychain regression; it does not imply M2–M5 or iPhone acceptance.

## Source and flow

The exact public portal base blob `67bd37ffafa672a98abb6d405c777c236aa80193` was read alongside the verified R2.1 cumulative patch (SHA256 `62e983d25e42a6318140797cc42f14e48800bcd2ff71c36f824f8245041aac13`). The patch leaves `createPairing` and `pair` unchanged. The actual contract is a 32-character base64url code, one-use/expiring on the server, `POST /mobile/api/pair` with only `code`, exact Origin, HTTPS, success HTTP200 JSON `authenticated:true` plus a single scoped server-issued Cookie. The response is not the full `/session` document. No admin API-key exchange, fabricated endpoints or server changes are added.

Native screen -> one-use Rust `PairingAttempt` -> same-origin HTTPS POST -> strict response/Cookie validation -> opaque SessionCredential -> existing Keychain/Keystore writer -> explicit authenticated GET /mobile/api/session -> authorized Topic keys.

Save precedes GET. If the GET fails, show SAVED_SESSION_UNCONFIRMED and retain the original expiry; Restore performs a read only, not a repeated pairing POST. The UI never stores the pairing code in saved instance state/UserDefaults/preferences. Only the non-secret last origin is stored outside the vault. Editing the origin clears previously displayed Topic metadata. No request runs just because the screen opens.

A pairing object atomically consumes its code once, including unknown network outcomes. Redirects and library-level retries are disabled; a new user action/code is required after an ambiguous consumed attempt. No old Cookie or Authorization header is attached to pairing. Requests/responses, pair codes, tokens and private histories are never logged. Credential TTL begins at request start, conservatively excluding network time. Vault generation checks fence late pairing responses after removal or replacement. Local removal is explicitly not remote logout, task cancellation or pending-submission deletion.

The pinned reqwest0.12.23 source exposes ClientBuilder::retry and defaults to protocol-NACK retries; both clients now explicitly use `retry::never` without changing Cargo.lock. Source: https://github.com/seanmonstar/reqwest/blob/v0.12.23/src/async_impl/client.rs

## Test levels

- Eight new Rust tests: seven offline contracts and one real loopback TLS/HTTP test spanning success, rejected, redirect, dropped response and duplicate Cookie cases. The TLS fixture creates ephemeral CA/leaf keys inside TempDir, verifies the hostname, counts exactly one request and is test-only. No disabled certificate verification or production trust override exists.
- Two Android and two iOS connection UI tests exercise invalid-code rejection/clearing and unpaired restore without network. The original four/six Android and three/six iOS UI/vault tests are unchanged. Android instrumentation includes all twelve methods; Xcode discovers eleven tests.
- Local Python verified the five fixture modes using real CA+hostname validation; Python/Swift parse and shell syntax passed. These are not local Rust/Android/Xcode test runs. This environment has no Cargo/Android/Xcode toolchains; the new code requires its own CI.
- The fixture is deliberately synthetic. Real NativeSync pairing/Topic history, submission ACK loss, server compatibility, device permissions and signed iPhone distribution remain separate, uncompleted gates.

## Next

Read this candidate's exact CI before declaring any new tests passed. Fix an actual failure without deleting old cases. Then connect the authenticated Topic history/submission/reconciliation to native application state, and validate against the exact isolated NativeSync service. Main, old client, backend/History/OneRing/AA authority, private source ZIPs, production and real history remain unchanged.
