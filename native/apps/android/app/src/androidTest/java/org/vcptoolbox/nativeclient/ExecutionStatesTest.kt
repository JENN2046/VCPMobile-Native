package org.vcptoolbox.nativeclient

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith

/** Runs on Android so org.json behavior is tested, not mocked on the JVM. */
@RunWith(AndroidJUnit4::class)
class ExecutionStatesTest {
    @Test fun onlyStateIsProjectedWithoutExecutionBodies() {
        val result = executionLabelsFromPage("""[{"id":"private","state":"running","extension":{"body":"secret"}},{"state":"review_required"}]""")
        assertEquals(listOf("服务端状态：running", "服务端状态：review_required"), result)
        assertFalse(result.joinToString().contains("secret"))
        assertFalse(result.joinToString().contains("private"))
    }
    @Test fun malformedMixedAndOversizedStatesFailClosed() {
        assertEquals(listOf("任务状态快照无效，请在服务端核对"), executionLabelsFromPage("not json"))
        assertEquals(listOf("任务状态快照无效，请在服务端核对"), executionLabelsFromPage("""[{"state":"running"},7]"""))
        assertEquals(listOf("服务端状态：未识别"), executionLabelsFromPage("""[{"state":"x\\nsecret"}]"""))
        assertEquals(listOf("任务状态快照过大，请在服务端核对"), executionLabelsFromPage("[" + " ".repeat(131_072) + "]"))
    }
    @Test fun stateProjectionIsVisuallyBounded() {
        val states = (0 until 28).joinToString(",") { """{"state":"running"}""" }
        val rendered = executionLabelsFromPage("[$states]")
        assertEquals(25, rendered.size)
        assertEquals("另有 4 项状态未显示", rendered.last())
    }
}
