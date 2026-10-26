# M2 platform secure Cookie vault (candidate for CI verification)

This is a **selective addition** from the retained LOCAL_UNCOMMITTED candidate, not an overwrite of the later one-shot HTTPS POST or service policy. NativeSync R2.1 server remains the only session/Topic/task authority. No production URL, pairing code, secret, cookie or private source archive is embedded.

## Android
- `PortalSessionVault.kt`: API 26+ Android Keystore AES-256-GCM, random IV, authenticated origin-specific associated data, atomic ciphertext in application noBackupFilesDir. Never stores plaintext on disk or exports a Keystore key.
- A lost Keystore key, malformed stored record, invalid HTTPS origin, or MAC failure is surfaced as a fixed non-sensitive error. The vault never silently regenerates an existing key or retries a submission.
- `PortalVaultTest.kt`: six Android emulator instrumentation cases run individually with hard time bounds, in addition to the unchanged four existing UI cases. Actual pass/fail depends on the new revision's CI.

## iOS
- `PortalSessionVault.swift`: app-only, non-synchronizing Keychain `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`. The caller must request a read explicitly; a missing or bad entry does not trigger pairing or another request.
- `PortalVaultTests.swift`: six separate app-host Keychain tests plus the three original UI tests via a new XcodeGen unit-test target. Simulator success is not iPhone hardware acceptance.

## Shared rules
- `SessionCredential` is created only after an independently verified HTTPS Set-Cookie response. Supplying an origin argument is not proof of TLS or pairing. The vault stores bounded opaque credential bytes with original issue and expiry times; restoration never renews TTL.
- Expiry, origin, port, invalid records and logout stay reject-only. `fetchSession` is an explicit, read-only GET /mobile/api/session using the current Rust client; app startup invokes neither GET nor POST.
- Local `remove` is not a server logout and never clears pending/unknown submissions. `fetchSession` has no execution, retry, or AA task authority. Cross-process/physical device and real authenticated service tests remain separate acceptance work.
- Do not log, back up, sync, or render the Cookie. HTTP reqwest header sensitivity applies to GET and the existing one-shot POST.
- Tests are **not passed** simply because the source files exist. Read the exact new GitHub Actions job logs, iOS simulator summary and per-method Android evidence. Production was not accessed.
