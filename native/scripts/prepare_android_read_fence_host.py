#!/usr/bin/env python3
"""Emit JVM tests of extracted production async methods, with controlled I/O.

Usage: python prepare_android_read_fence_host.py OUTPUT_DIRECTORY
Add that directory to the existing tests/kotlin compileTestKotlin source task.
Uses real generated UniFFI record types; the vault and lifecycle host are fake.
This is NOT Android lifecycle/Keystore, JSON parser or device evidence. The
decoder trap ensures a stale history callback cannot even reach projection.
"""
from pathlib import Path
import hashlib
import json
import sys
from check_lifecycle_read_fence import section

root = Path(__file__).resolve().parents[1]
source_path = root / 'apps/android/app/src/main/java/org/vcptoolbox/nativeclient/ConnectionScreen.kt'
source = source_path.read_text(encoding='utf-8')
output = Path(sys.argv[1])
output.mkdir(parents=True, exist_ok=True)
methods = '\n'.join(section(source, marker) for marker in [
    '    fun inspectPending()', '    fun readHistory(',
    '    private fun cancelLocalReads()', '    fun suspendForBackground()',
    '    fun stopChat()', '    fun cancelRecovery()', '    private fun clearHistory(',
    '    fun selectTopic(', '    fun confirmSend(',
])
prefix = r'''
package org.vcptoolbox.nativeclient
import kotlinx.coroutines.*
import kotlin.coroutines.*
import org.junit.Assert.*
import org.junit.Test
import org.vcptoolbox.nativecore.*

// Any stale history response reaching a decoder is an assertion failure, not
// a replacement parser or a claim about Android's JSON implementation.
private class JSONObject(raw: String) {
    init { throw AssertionError("stale response reached decoder: $raw") }
    fun opt(key: String): Any? = null
}
private fun executionLabelsFromPage(raw: String): List<String> =
    throw AssertionError("stale response reached execution projection")

private class ControlledVault {
    lateinit var pending: Continuation<List<NativePendingPreview>>
    lateinit var history: Continuation<NativeHistoryPage>
    suspend fun listPending(origin: String, expected: NativeSessionInfo, topic: String): List<NativePendingPreview> =
        suspendCoroutine { pending = it } // deliberately ignores Job cancellation
    suspend fun fetchHistory(origin: String, expected: NativeSessionInfo, topic: String, after: ULong): NativeHistoryPage =
        suspendCoroutine { history = it }
}
private class Probe {
    val viewModelScope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
    val vault = ControlledVault()
    var session: NativeSessionInfo? = NativeSessionInfo("a".repeat(64), "b".repeat(64), listOf("topic_a", "topic_b"), null)
    var origin = "https://fixture.invalid"
    var topics = listOf("topic_a", "topic_b")
    var selectedTopic: String? = "topic_a"
    var viewGeneration = 1L
    var status = "SESSION_READY"
    var busy = false
    var pendingRows = emptyList<NativePendingPreview>()
    var pendingReviewed = true
    var pendingStatus = "PENDING_EMPTY"
    var unresolvedSend = false
    var pendingInventoryJob: Job? = null
    var historyJob: Job? = null
    var historyRows = emptyList<NativeHistoryRow>()
    var historyCursor = 0uL
    var historyHasMore = false
    var historyStatus = "HISTORY_READY"
    var historyExecutable = true
    var executionStates = emptyList<String>()
    var recoveryFence: NativeRecoveryFence? = null
    var recoveryJob: Job? = null
    var recoveryActive = false
    var recoveryStatus = "RECOVERY_IDLE"
    var chatFence: NativeRecoveryFence? = null
    var chatJob: Job? = null
    var chatActive = false
    var sendingOriginal = false
    var sendStatus = "SEND_IDLE"
    var replyStatus = "REPLY_IDLE"
    var draft = "draft"
    var outgoingText = ""
    var submits = 0
    fun canConfirmSend(ticket: NativeSendConfirmation): Boolean = ticket.matches(
        origin, session?.entryId, session?.sessionMarker, selectedTopic, viewGeneration, draft)
    fun sendMessage() { submits++ } // counts admission attempts, never a real POST
    fun expectedPage() = NativeHistoryPage(session!!.entryId, session!!.sessionMarker,
        "topic_a", 0uL, 1uL, false, listOf(NativeHistoryRow(1uL,"m",false,"{}")), "[]", "{}")
'''
suffix = r'''
}
class DirectedReadFenceHostTest {
    @Test fun failedReauthenticationRevokesPriorEmptyGrant() {
        val x=Probe(); x.inspectPending()
        assertFalse(x.pendingReviewed)
        x.vault.pending.resumeWithException(IllegalStateException("offline denial"))
        assertFalse(x.pendingReviewed); assertEquals("PENDING_FAILED",x.pendingStatus)
        assertFalse(x.busy)
    }
    @Test fun onlyCurrentEmptyInventoryGrants() {
        val x=Probe(); x.inspectPending(); x.vault.pending.resume(emptyList())
        assertTrue(x.pendingReviewed)
        x.inspectPending(); assertFalse(x.pendingReviewed)
        x.vault.pending.resume(listOf(NativePendingPreview("r","topic_a","message","UNKNOWN","query")))
        assertFalse(x.pendingReviewed); assertTrue(x.unresolvedSend)
    }
    @Test fun backgroundLateInventoryCannotClearNewReadOrGrant() {
        val x=Probe(); x.inspectPending(); val old=x.vault.pending
        x.suspendForBackground(); assertFalse(x.pendingReviewed)
        x.inspectPending(); val fresh=x.vault.pending
        old.resume(emptyList())
        assertFalse(x.pendingReviewed); assertTrue(x.busy)
        fresh.resume(emptyList()); assertTrue(x.pendingReviewed); assertFalse(x.busy)
    }
    @Test fun backgroundLateHistoryCannotProjectOrReleaseNewBusyState() {
        val x=Probe(); val page=x.expectedPage(); x.readHistory(); val old=x.vault.history
        x.suspendForBackground(); x.inspectPending()
        old.resume(page)
        assertTrue(x.historyRows.isEmpty()); assertEquals(0uL,x.historyCursor)
        assertFalse(x.historyExecutable); assertTrue(x.busy)
        assertEquals("HISTORY_RECHECK_REQUIRED",x.historyStatus)
        x.vault.pending.resume(emptyList())
    }
    @Test fun lateInventoryCannotCrossSessionOrTopic() {
        for (change in listOf<(Probe)->Unit>(
            { it.session=it.session!!.copy(sessionMarker="c".repeat(64)) },
            { it.selectedTopic="topic_b" })) {
            val x=Probe(); x.inspectPending(); change(x); x.vault.pending.resume(emptyList())
            assertFalse(x.pendingReviewed); assertTrue(x.pendingRows.isEmpty())
        }
    }
    @Test fun stopChatSynchronouslyInvalidatesOldView() {
        val x=Probe(); val fence=NativeRecoveryFence(x.session!!,"topic_a",1uL)
        try {
            x.chatFence=fence; x.chatJob=Job(); x.chatActive=true; x.busy=true
            x.stopChat(); assertTrue(x.viewGeneration>1L); assertNull(x.chatFence)
            assertFalse(x.historyExecutable); assertFalse(x.chatActive); assertFalse(x.busy)
        } finally { fence.destroy() }
    }
    @Test fun sameConfirmationCannotBeConsumedTwiceAfterRecheck() {
        val x=Probe(); val s=x.session!!
        val ticket=NativeSendConfirmation(x.origin,s.entryId,s.sessionMarker,"topic_a",1L,x.draft)
        x.confirmSend(ticket); assertEquals(1,x.submits)
        x.pendingReviewed=true // a later successful explicit empty review
        x.confirmSend(ticket); assertEquals(1,x.submits)
    }
}
'''
(output/'DirectedReadFenceHostTest.kt').write_text(prefix+methods+suffix,encoding='utf-8')
files = [source_path]
for name in ['HistoryWindow','NativeTopicSendGuard','NativeSendConfirmation']:
    p=source_path.parent/(name+'.kt'); files.append(p)
    (output/p.name).write_bytes(p.read_bytes())
(output/'SOURCE_IDENTITY.json').write_text(json.dumps({str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in files},indent=2),encoding='utf-8')
print('Generated 7 controlled-I/O JVM tests from production methods; not device evidence:',output)
