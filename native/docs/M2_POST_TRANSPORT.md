# M2 explicit one-time POST transport (2026-10-26)

Status: isolated Rust implementation, new CI pending. Not wired to mobile apps, secure vault or actual NativeSync test service.

Accepted source: NativeSync R2.1 portal.js after-blob 6950b91ca70b4f161906e28036b9ccec589e73a5. Only session-authorized Topic paths from first_attempt are permitted. A server-issued __Secure-vcp_mobile Cookie on the exact pinned HTTPS origin and exact Origin/X-NativeSync-Entry-Id/X-NativeSync-Session-Marker headers are mandatory.

Workflow: caller first fetches an authenticated session using the read-only client, obtains the Cookie through platform protected storage, and prepares original JSON. first_attempt validates and durably claims the original request in SQLite before any network request. OutboundSubmission is non-Clone with private identity/path fields. submit_claimed_once consumes it. No auto-retry, redirects, inherited proxies or automatic Cookie jar. Response size is bounded to 128KiB; unexpected Set-Cookie, URL change or TLS/timeout error fails closed. HTTP202 is AcceptedButUnverified, NEVER an execution receipt and NEVER pending cleanup. HTTP 409 or other unknown failures require original receipt query, not new identity or automatic resubmit. Cross-session recovery remains query-only.

Synthetic unit tests inspect the reqwest POST request, exact original bytes, guards, Cookie, persistent claim and cross-origin rejection. These are NOT real server calls or true ACK-loss tests. Keychain/Keystore, Cookie expiry, isolated service integration, encrypted data handling, source-verified response parsing and device testing remain. M2–M5 are not accepted from this code alone.
