# M2 HTTP and session-cookie policy (2026-10-26)

The source-verified R2.1 portal sets __Secure-vcp_mobile as a 43-byte base64url token with Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=N, and revokes with Max-Age=0. The server cookie is authority; entryId and sessionMarker are only reject-only guards.

New core/src/transport_policy.rs enforces exact TLS-origin scope (caller must attest real HTTPS URL), cookie attributes and revocation, secret-redacted Debug, safe /mobile/api routes, bounds on session/ACK responses, authenticated session parsing and conservative status classification. HTTP 202 is AcceptedButUnverified and never clears a PendingStore row without separate authenticated history receipt. 409 must carry exact SUBMISSION_CONTEXT_CHANGED code. 401/403/redirects/unknown 5xx never cause auto-resend.

Not implemented: real TLS transport, protected host cookie vault, expiry enforcement, pairing, actual server checks, persistent credential storage or physical-device verification. The future iOS/Android host MUST provide exact host/path-scoped Keychain/Keystore storage, TLS hostname verification, no redirects and no secret logging. No private source ZIP has been published.

Tests cover issuance/revocation, hostile attributes/cross-origin/unsafe paths, session body provenance preconditions and safe response status handling. Only the new exact CI revision can validate their execution.

## Read-only network implementation slice

The new core/src/https_transport.rs implements the HTTPS GET session path with pinned DNS TLS origin, no redirect/proxy and bounded streaming response. Unexpected session Set-Cookie is rejected until protected native vault handling supports rotation and revocation. No real service call or credentials occurred, and this remains an unverified candidate until its own CI passes. M2 is NOT complete.
