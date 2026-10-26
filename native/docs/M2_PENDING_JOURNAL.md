# M2 explicit native pending inventory (development batch)

This feature is a **read-only recovery inventory**, not message submission or receipt confirmation.
It is intentionally reachable only after a deliberate Topic selection and button press.
It never POSTs, replays, deletes pending rows, cancels tasks, or treats an HTTP 202
or execution-state string as completed AA work.

## Trust and lifecycle

- Android supplies its application-private `noBackupFilesDir/portal-pending-v1`;
  iOS supplies Application Support `portal-pending-v1`, marks the directory
  excluded from backup and requests device file protection.
- The Rust journal accepts only canonical HTTPS origins and an existing absolute
  non-symlink directory. Each origin has its own hashed SQLite file; unrelated
  database schemas and corrupt records are **not reset or erased**. This is
  a private app directory supplied by trusted native code, never user input.
- A single deliberate inventory action restores a protected Cookie, makes an
  authenticated `GET /mobile/api/session`, verifies the entry/session/Topic
  against the displayed authorized context, then lists at most 16 scoped
  local records. Both platform vaults fence credential replacements and
  removal using their in-memory epochs; both screens also fence Topic/view
  generation. Original message payload bytes do not reach the display.
- `PREPARED_NOT_SENT` means no durable initial attempt claim; it does not
  authorize an implicit send. `AWAITING_RECEIPT` means the initial local
  claim exists, **not** that the server accepted it. Cross-session rows are
  always `QUERY_ONLY_CROSS_SESSION`, not remotely controllable.
- Reopening an existing per-origin database does not create, resend, or alter
  its pending submissions. Opening a fresh journal creates an empty private
  SQLite file; this is local metadata, not a network or task write.

## Evidence and residual gap

Four Rust offline tests cover authorization/Topic separation, same-vs-new-session
recovery labels, origin-isolated reopen, foreign database retention, and symlink
directory refusal. The Android/iOS unpaired UI checks now additionally require
the new inventory action to remain absent. All existing tests remain enabled.

The owning message submission flow is **not yet wired to either native UI**.
Thus an empty inventory is expected for current builds; it must not be
misrepresented as evidence that old NativeSync server receipts were checked.
The Rust authenticated receipt gate remains internal and does not commit after
a potentially stale platform credential epoch. Next: create actual per-origin
pending submission records before one initial POST, expose explicit receipt
read/confirmation with credential-epoch protection, then test the exact
NativeSync R2.1 service in isolation. No production, model, AA or credential
side effects are authorized in these synthetic tests.
