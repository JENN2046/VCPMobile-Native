package org.vcptoolbox.nativeclient

import org.junit.Assert.*
import org.junit.Test

class NativeExecutionLabelsTest {
    @Test fun showsOnlyBoundedServerStateTokens() {
        assertEquals(listOf("服务端状态：running", "服务端状态：review_required", "服务端状态：未识别"),
            NativeExecutionLabels.render(listOf("running", "review_required", null), 3))
    }

    @Test fun invalidOrSecretLikeStringsCannotBeDisplayed() {
        val raw = listOf("line\nsecret", "token=123", "🔥", "x".repeat(49), "")
        assertEquals(List(raw.size) { "服务端状态：未识别" }, NativeExecutionLabels.render(raw, raw.size))
    }

    @Test fun excessExecutionsAreCountedButNeverDecoded() {
        val states = List(24) { "queued" }
        val preview = NativeExecutionLabels.render(states, 100)
        assertEquals(25, preview.size)
        assertEquals("另有 76 项状态未显示", preview.last())
    }

    @Test fun outOfRangeProjectionFailsClosed() {
        assertThrows(IllegalArgumentException::class.java) {
            NativeExecutionLabels.render(listOf("running"), 0)
        }
        assertThrows(IllegalArgumentException::class.java) {
            NativeExecutionLabels.render(List(25) { "running" }, 25)
        }
    }
}
