import org.vcptoolbox.nativecore.*
import kotlinx.coroutines.runBlocking
import org.junit.Test
import kotlin.test.*
import java.util.concurrent.CopyOnWriteArrayList

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
}
