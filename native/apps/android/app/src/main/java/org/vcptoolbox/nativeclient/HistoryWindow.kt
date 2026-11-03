package org.vcptoolbox.nativeclient

import org.vcptoolbox.nativecore.NativeHistoryRow

/** View-only upserts: a later central-history revision replaces the same
 * message ID, including deletion. It never confirms a request or AA outcome. */
internal fun mergeHistoryWindow(previous: List<NativeHistoryRow>, incoming: List<NativeHistoryRow>): List<NativeHistoryRow> {
    // Source parser rejects same-page repeated IDs; keep this gate at the
    // view boundary so a bad/changed binding cannot silently collapse them.
    require(incoming.map { it.messageId }.distinct().size == incoming.size) { "Duplicate history page identity" }
    val byId = previous.associateBy { it.messageId }.toMutableMap()
    for (row in incoming) {
        val old = byId[row.messageId]
        require(old == null || row.sequence > old.sequence) { "Stale history revision" }
        byId[row.messageId] = row
    }
    return byId.values.sortedBy { it.sequence }.takeLast(400)
}
