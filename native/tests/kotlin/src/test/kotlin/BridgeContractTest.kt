import org.vcptoolbox.nativecore.*
import kotlinx.coroutines.runBlocking
import org.junit.Test
import kotlin.test.*
import java.util.concurrent.CopyOnWriteArrayList
import java.nio.file.Files

class BridgeContractTest {
    private class Recorder(private val cancel: (() -> Unit)? = null) : ProbeListener {
        val events = CopyOnWriteArrayList<UInt>()
        override fun onProgress(progress: ProbeProgress) {
            events.add(progress.completed)
            cancel?.invoke()
        }
    }

    @Test fun successAndOrderedCallbacks(): Unit = runBlocking {
        val probe = BridgeProbe()
        try {
            val listener = Recorder()
            val result = probe.run(ProbeRequest("跨端 probe 🧭", 3u, 0u), listener)
            assertEquals("跨端 probe 🧭", result.label)
            assertEquals(3u, result.completed)
            assertEquals(listOf(1u, 2u, 3u), listener.events.toList())
        } finally { probe.destroy() }
    }

    @Test fun structuredFailureCrossesBinding(): Unit = runBlocking {
        val probe = BridgeProbe()
        try {
            val error = assertFails {
                probe.run(ProbeRequest("probe", 0u, 0u), Recorder())
            }
            assertTrue(error.toString().contains("steps"), error.toString())
        } finally { probe.destroy() }
    }

    @Test fun reentrantCancellationCrossesBinding(): Unit = runBlocking {
        val probe = BridgeProbe()
        try {
            val listener = Recorder { probe.cancel() }
            val error = assertFails { probe.run(ProbeRequest("cancel", 10u, 1u), listener) }
            assertTrue(error.toString().contains("cancel", ignoreCase = true), error.toString())
            assertEquals(listOf(1u), listener.events.toList())
        } finally { probe.destroy() }
    }

    @Test fun oneShotLifecycleIsEnforced(): Unit = runBlocking {
        val probe = BridgeProbe()
        try {
            probe.run(ProbeRequest("first", 1u, 0u), Recorder())
            assertFails { probe.run(ProbeRequest("second", 1u, 0u), Recorder()) }
        } finally { probe.destroy() }
    }

    @Test fun pendingRecoveryReopensAndCancelledQueryDoesNotUseNetwork(): Unit = runBlocking {
        val root = Files.createTempDirectory("vcp-native-recovery-").toFile()
        val origin = "https://recovery-binding.example.test"
        val expected = NativeSessionInfo("a".repeat(64),"b".repeat(64),listOf("topic_a"),null)
        val journal = NativePendingJournal.openScoped(origin,root.absolutePath)
        val original = try { journal.prepareOriginal(expected,"topic_a",NativePendingKind.MESSAGE,
            "{ \"requestId\":\"binding_pending_123\", \"text\":\"synthetic\" }") }
            finally { journal.destroy() }
        val reopened = NativePendingJournal.openScoped(origin,root.absolutePath)
        val credential = SessionCredential.fromServerCookie(origin,origin,
            "__Secure-vcp_mobile=${"B".repeat(43)}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600")
        val fence = NativeRecoveryFence(expected,"topic_a",3uL)
        try {
            assertEquals(listOf(original),reopened.pendingForTopic("topic_a"))
            fence.cancel()
            assertFailsWith<NativeRecoveryException.Cancelled> { credential.queryPendingRecovery(reopened,original,fence) }
            assertEquals(listOf(original),reopened.pendingForTopic("topic_a"))
        } finally { fence.destroy(); credential.destroy(); reopened.destroy(); root.deleteRecursively() }
    }
    @Test fun recoveryFenceRejectsUnauthorizedTopicLocally() {
        val expected = NativeSessionInfo("a".repeat(64),"b".repeat(64),listOf("topic_a"),null)
        assertFailsWith<NativeRecoveryException.Forbidden> { NativeRecoveryFence(expected,"topic_other",1uL) }
    }
    @Test fun sharedUtf16PolicyCrossesBinding() {
        assertEquals(4000u,messageInputMaxUtf16())
        for (text in listOf("中".repeat(4000),"🧭".repeat(2000),"中".repeat(3998)+"🧭")) {
            assertTrue(isMessageTextValid(text))
        }
        for (text in listOf("中".repeat(4001),"🧭".repeat(2001),"中".repeat(3999)+"🧭"," \uFEFF\u3000")) {
            assertFalse(isMessageTextValid(text))
        }
    }
    @Test fun compatibilityEntriesShareOneStoreAndLegacySenderExplicitlyRefuses(): Unit = runBlocking {
        val root=Files.createTempDirectory("vcp-native-one-store-").toFile()
        val origin="https://one-store-binding.example.test"
        val expected=NativeSessionInfo("a".repeat(64),"b".repeat(64),listOf("topic_a"),null)
        val first=NativePendingJournal(origin,root.absolutePath)
        val second=NativePendingJournal.openScoped(origin,root.absolutePath)
        val credential=SessionCredential.fromServerCookie(origin,origin,
            "__Secure-vcp_mobile=${"B".repeat(43)}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600")
        try {
            assertEquals(first.privatePath(),second.privatePath())
            val original=first.prepareOriginal(expected,"topic_a",NativePendingKind.MESSAGE,
                "{\"requestId\":\"binding_original_123\",\"text\":\"synthetic\"}")
            assertEquals(listOf(original),second.pendingForTopic("topic_a"))
            assertFailsWith<NativeSendException.GuardedSendRequired> {
                second.sendOriginalMessageOnce(credential,expected,"topic_a","other_original_123","different")
            }
            assertEquals(listOf(original),second.pendingForTopic("topic_a"))
            assertFailsWith<NativeRecoveryException.StorageRejected> {NativePendingJournal.open(origin,root.resolve("escape.sqlite").path)}
            assertFalse(root.resolve("escape.sqlite").exists())
        } finally {credential.destroy();second.destroy();first.destroy();root.deleteRecursively()}
    }

}
