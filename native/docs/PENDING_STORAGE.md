# Pending submission storage boundary

This is a local building block, not a NativeSync transport. It does not claim R2.1 compatibility without the exact accepted source and corresponding integration tests.

- `prepare` validates and commits the original payload before transport. It retains exact bytes; no invented request_hash or normalization.
- `mark_attempted` commits before dispatch. Missing or damaged local storage must prevent the caller from sending.
- Recovery queries first. Same-session retry still needs the existing explicit retry decision. A new session gets query-only; never automatically resubmit an old request or mint a replacement identity.
- The local key is endpoint + non-credential original-session continuity marker + request ID. It is not a global request ID or an access credential.
- Confirmation is local cleanup only after the authenticated adapter checks original ownership, Topic, kind and payload against the real server. It does not assert AA success or grant control of an old task.
- SQLite IMMEDIATE transactions serialize read-modify-write across connections. The in-process mutex protects only one connection; no application-global lock.
- Capacity is 16 pending records / 256 KiB original payload bytes. Overflow fails visibly, never discards an unknown submission. Confirmed payloads are logically removed. SQLite physical erasure is not guaranteed by DELETE.
- An unrelated/future database is refused, never reset. Use an isolated private application-support path, not a user-supplied production history path. Before real sensitive data, integrate platform file protection/encryption and retention choices.
- Run synchronous disk operations off the UI thread. No credential field or network mechanism is included. Caller remains responsible for keeping credentials out of payloads and diagnostics.

Fourteen tests cover raw-byte idempotence, changed identity payload rejection, namespace isolation, reopen recovery, cross-session query-only decisions, exact cleanup, count/UTF-8 byte limits, malformed input, unrelated/future databases, concurrent connections, retained corrupt records, missing paths and a real child process exiting after commit without connection destruction. Physical power loss, true HTTP ACK loss and complete NativeSync compatibility require separate actual evidence. Tests written are not tests passed; see the exact CI revision.
