# M2 read-only Topic history view (incremental)

Development candidate against `dce262d03f36802e8c66f99953e9605f8fdeea14`.

- Android Compose and iOS SwiftUI offer an explicit Topic selection and **manual** page read after a real verified session. App launch, opening the connection tab, Topic selection and restoration alone never fetch messages.
- Every page is obtained from the previously implemented platform-protected session credential, through the exact authenticated `fetchHistoryPage` contract. Connection, Topic and session generation checks reject late results; server pages are validated in Rust; no user message, cookie, task state or raw JSON is logged.
- Render only a bounded plain-text preview of native text messages (400 characters); non-text payloads and tombstones are clearly placeholders. HTML/script is not interpreted. Keep at most 400 rows **in memory only**; a Topic switch, Origin edit, removal, reconnect/pair or explicit re-read drops the local cache. No disk cache or offline authority is implied.
- Execution receipts are deliberately not displayed as completed jobs and not used to clear or replay pending submissions. Every history fetch is a GET, never a POST, with no model dispatch. The UI remains a read-only engineering preview, not M3 complete chat.
- Existing simulator tests are preserved; extra assertions check unpaired screens cannot expose a history read. No fixture forces a fake authenticated session. CI verification and real NativeSync testing are separate acceptance gates.

Follow-up after this candidate's own CI: source-bound exact original-pending receipt reconciliation with bounded durable storage, then actual isolated NativeSync service pairing/history/ACK-loss, and M3 chat/streaming/attachment/AA UI.
