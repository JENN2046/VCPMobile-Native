package org.vcptoolbox.nativeclient

import org.junit.Assert.*
import org.junit.Test
import org.vcptoolbox.nativecore.NativeHistoryRow

class HistoryWindowTest {
    private fun row(sequence: Int, id: String = "reply", deleted: Boolean = false) =
        NativeHistoryRow(sequence.toULong(), id, deleted, if (deleted) null else "{\"content\":\"revision $sequence\"}")
    @Test fun centralReplyUpdatesAndDeletionReplaceTheSameMessage() {
        val first = mergeHistoryWindow(emptyList(), listOf(row(1)))
        val update = mergeHistoryWindow(first, listOf(row(2)))
        assertEquals(1, update.size)
        assertEquals(2uL, update.single().sequence)
        val deletion = mergeHistoryWindow(update, listOf(row(3, deleted = true)))
        assertTrue(deletion.single().deleted)
        assertNull(deletion.single().messageJson)
    }
    @Test fun staleRevisionsCannotOverwriteLaterContent() {
        assertThrows(IllegalArgumentException::class.java) {
            mergeHistoryWindow(listOf(row(9)), listOf(row(8)))
        }
        assertThrows(IllegalArgumentException::class.java) {
            mergeHistoryWindow(listOf(row(9)), listOf(row(9)))
        }
    }
    @Test fun boundedWindowPreservesSequenceWithoutDroppingTheLatestRevision() {
        val rows = mergeHistoryWindow(emptyList(), (1..500).map { row(it, "m_$it") })
        assertEquals(400, rows.size)
        assertEquals(101uL, rows.first().sequence)
        assertEquals(500uL, rows.last().sequence)
    }
}
