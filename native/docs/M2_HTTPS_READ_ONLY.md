# M2 Read-only HTTPS transport

This is the first actual network implementation, deliberately limited to authenticated GET /mobile/api/session. It is called only by an explicit Rust caller and is not yet connected to either mobile screen.

- Origin must be canonical DNS HTTPS with no path, userinfo or IP literal.
- Redirects, automatic Cookie storage and inherited proxies are disabled. No HTTP downgrade allowed.
- Cookie is from an in-memory, source-validated PortalCookie on precisely the same origin and only the /mobile/api/session route.
- An unexpected Set-Cookie from the session response fails closed until Keychain/Keystore rotation and revocation semantics are implemented.
- Both Content-Length and streaming chunk accumulation enforce a 512 KiB limit.
- Only HTTP 200 JSON parsed into an authenticated R2.1 session yields success. No POST, model call, task control, history write or automatic retry.
- Unit tests cover actual reqwest request construction and rejection of untrusted origins; they do not emulate successful live TLS to the service.
- The Rust native client has no credential vault and no pairing. iOS and Android secure stores, real authenticated server fixture, session renewal, submit/ACK reconciliation and physical devices are pending.
- Added pinned reqwest 0.12.23 Rustls+stream dependency, so the old Cargo.lock digest is obsolete. Resolve a new lock from actual CI, verify its identity and commit it as a distinct reproducibility step.

## Topic history read-only slice (requires new CI evidence)

The next code slice adds `PortalHttpClient::fetch_history` for source-confirmed `GET /mobile/api/conversations/:key?after=<cursor>`. Only a `PortalSession`-authorized Topic, JS-safe cursor and same pinned Cookie origin can build the request. The client enforces HTTP 200 JSON object, exact response URL, no Set-Cookie, Content-Length/streaming 2 MiB bound and no redirect/proxy. Returned `HistorySnapshot` retains entry/session/Topic/cursor provenance, but deliberately does not parse/apply messages, update a cursor, clear pending state or dispatch POST. It is not a validated execution receipt. No actual service was contacted and this commit requires its own CI.
