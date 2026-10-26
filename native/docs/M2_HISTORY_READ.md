# M2 authenticated history read

Candidate based on `5b65ae66511c2da0e6459179f5f0115cc92c3f05`. This work is not part of run40296683389 and has no Rust/mobile compile or live-service PASS yet.

## Exact source basis

Accepted portal after-blob `6950b91ca70b4f161906e28036b9ccec589e73a5`, verified cumulative patch, and accepted PWA App.vue blob `c909ef6b2b872e5d4c633344f27f1b8183e5e067`. The history route itself is unchanged by that patch: authenticated GET `/mobile/api/conversations/:key?after=...`; SQL `last_seq > after ORDER BY last_seq LIMIT 201`, first 200 message envelopes, `next` equals last emitted sequence (or the input cursor), `hasMore` indicates a 201st row. Executions are a separate unpaginated projection. `topicState` includes historyReady and executable. The response does not echo signed session context: request tags are local provenance, not new server authority.

## Implemented scope

- `history.rs` returns a typed page only after validating all envelopes, monotonic safe-integer cursors, distinct message identities, tombstones and paging flags. Malformed pages do not partially advance a cursor. Native message JSON and execution JSON remain separate; opaque extensions are retained structurally, not executed as HTML. Rust Debug formatting redacts message/task/context data; hosts must never log raw page or generated-record descriptions.
- `SessionCredential.fetch_history_page` checks credential expiry and local request shape before network, re-reads the authenticated session, rejects changed entry/session or unauthorized Topic, then reads one history page through the existing scoped HTTPS client. It never POSTs, retries a submission, mutates history or confirms pending records. An app applies results only after checking its view/Topic generation.
- HTTP401/403 remain explicit authentication/authorization failures rather than success or an empty history.
- iOS and Android vault wrappers keep their credential epoch checks; a late reply cannot restore a removed/replaced credential view. They do not automatically call this path at startup.

Eleven new Rust tests have been written (nine envelope cases, two credential bridge cases). One unpaired-history preflight test is added on each platform; the Android invocation list includes it explicitly. All prior test methods/assertions and Cargo manifests/lock are unchanged. Rust/UniFFI/mobile compilation is pending; shell/whitespace/Swift parsing is not a substitute. Native UI rendering, message caching, exact pending reconciliation, actual NativeSync service integration and ACK-loss acceptance remain separate follow-up work.

## Validation

The base run40296683389 has now passed both platforms (Android12/12, iOS11/11); it does not include this API. Run this revision's own CI before treating its new APIs as validated. Inspect exact HEAD before submission and do not overlap active CI. No production data, original private ZIPs, credentials, main branch or server execution authority are changed.
