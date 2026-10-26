# M2 native initial message attempt (historical seam description)

The local directed integration now explicitly rejects this unguarded legacy
ABI; mobile adapters use the single protected prepare/claim/dispatch pipeline.
See [current local contract](DIRECTED_CHAT_INTEGRATION.md). The description
below records the earlier seam and is not the current callable send contract.

The native pending journal now exposes an explicit `send_original_message_once`
Rust/UniFFI entry point. Neither mobile screen calls it yet. A future deliberate
Send button must first check platform credential epoch, current Origin, active
Topic, and view generation. Never call on startup, history refresh, app restore,
or a background retry. The caller supplies one random, stable requestId (8–80
ASCII [A-Za-z0-9_-]); do not regenerate an ID to escape unknown state.

## Operation

1. Check credential belongs to the journal's canonical HTTPS Origin.
2. Verify the expected entry/session/authorized Topic and fetch a fresh real
   cookie-authenticated HTTPS session with the same identity. No POST yet.
3. Construct the exact original JSON (`requestId`, `text`, `attachments:[]`).
   Reject invalid content and reserve the entry/Topic transactionally in the
   origin-scoped app-private SQLite journal. Persist full bytes then atomically
   claim the one initial attempt BEFORE the network call.
4. Send only that consumed intent using the existing secure-cookie, exact
   Origin, entry/session headers and no-redirect/no-retry HTTPS POST adapter.
   On every transport failure, session change, HTTP rejection or unknown outcome,
   retain the original pending record. Even HTTP202 returns
   `HTTP_202_UNVERIFIED`, NOT task completion. No execution state is inferred.
5. Future code must reuse the existing authenticated exact-receipt gate
   `reconcile_original_pending` before clearing a local row, and distinguish
   cross-session query-only from any same-session explicit retry.

## Boundaries and tests

Two offline tests cover durable once-only Topic admission, restart retention,
no content leakage in result/debug and invalid no-POST inputs. Existing POST
transport tests cover exact headers/body and no implicit retry. Only GitHub CI
on the exact new SHA can establish Rust/UniFFI/mobile compilation. No synthetic
fixture is claimed as live NativeSync service validation. Android/iOS UI send,
credential-epoch fencing during the privileged send transition, ACK-loss and
actual isolated R2.1 service acceptance remain unfinished. This code must not
be presented as a ready-to-use chat composer.
