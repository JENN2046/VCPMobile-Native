package org.vcptoolbox.nativeclient

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class NativeSendConfirmationTest {
    private val original = NativeSendConfirmation("https://one.example.test", "entry-1", "session-1",
        "topic-1", 17L, "first draft 🧣")

    @Test fun onlyExactOriginalContextAllowsConfirmation() {
        assertTrue(original.matches("https://one.example.test", "entry-1", "session-1",
            "topic-1", 17L, "first draft 🧣"))
        assertFalse(original.matches("https://two.example.test", "entry-1", "session-1",
            "topic-1", 17L, "first draft 🧣"))
        assertFalse(original.matches("https://one.example.test", "entry-2", "session-1",
            "topic-1", 17L, "first draft 🧣"))
        assertFalse(original.matches("https://one.example.test", "entry-1", "session-2",
            "topic-1", 17L, "first draft 🧣"))
        assertFalse(original.matches("https://one.example.test", "entry-1", "session-1",
            "topic-2", 17L, "first draft 🧣"))
        assertFalse(original.matches("https://one.example.test", "entry-1", "session-1",
            "topic-1", 18L, "first draft 🧣"))
        assertFalse(original.matches("https://one.example.test", "entry-1", "session-1",
            "topic-1", 17L, "modified draft 🧣"))
    }
}
