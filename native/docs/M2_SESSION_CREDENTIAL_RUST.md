# M2 native secure session-credential Rust bridge

This is one selectively merged component from the previously saved, uncommitted SessionVault candidate. It is not the Android Keystore/iOS Keychain vault itself, nor proof of a paired device or service authentication.

- A verified same-origin server Set-Cookie is parsed using the accepted NativeSync R2.1 cookie policy. The caller must independently validate the actual TLS response URL; a user-supplied response_origin string is not authentication.
- SessionCredential contains the opaque portal Cookie and original issuance/expiry timestamps. Restore validates a small, exact-version JSON record, canonical HTTPS origin, token shape, age and timestamp consistency without extending its TTL.
- credential_storage_key hashes the canonical origin with a domain-separated label. It is a lookup key, not a login credential, receipt, or authorization.
- secret_bytes_for_vault returns sensitive bytes solely for an Android Keystore/iOS Keychain implementation. Do not log, back up, synchronize, copy into pending SQLite/chat/history, or expose to a model.
- fetch_session_info invokes the existing pinned read-only HTTPS GET only when explicitly called, checks expiry before and after, and returns non-credential entry/session/Topic metadata. It performs no POST, AA actions, retries, background work or automatic recovery.
- Seven Rust tests check round-trip TTL preservation, expiry and clock rollback, origin/port scope, corrupt data, revocation, secret-redaction, and pre-network expiry.

Next: selectively merge the existing private candidate's Android Keystore and iOS Keychain host files on top of the current POST implementation, wire explicit user actions, then test protected storage on each supported simulator/device. Only actual CI results establish compilability. Real NativeSync pairing and cross-device continuity are separate acceptance gates.

## Host stores now added (CI pending)

Android: `apps/android/app/src/main/java/org/vcptoolbox/nativeclient/PortalSessionVault.kt` (Keystore AES-GCM, associated data, noBackup AtomicFile, fixed errors) and six standalone instrumentation cases. iOS: `apps/ios/Sources/PortalSessionVault.swift` (non-synchronizing, device-only Keychain actor) and six XcodeGen unit tests, in addition to pre-existing UI suites. See `M2_SESSION_VAULT.md`. They do not auto-run on launch, do not perform pairing, and do not invoke real VCP endpoints in the synthetic tests. Current revision CI required; no actual platform vault PASS yet.
