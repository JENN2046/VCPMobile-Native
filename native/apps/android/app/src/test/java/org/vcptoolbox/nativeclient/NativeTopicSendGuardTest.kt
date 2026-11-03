package org.vcptoolbox.nativeclient

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class NativeTopicSendGuardTest {
    @Test fun onlyExactReadyAndExecutableBooleanWithoutMoreHistoryAllowsSend() {
        assertTrue(NativeTopicSendGuard.allowsSend(true, true, false))
        assertFalse(NativeTopicSendGuard.allowsSend(true, true, true))
        assertFalse(NativeTopicSendGuard.allowsSend(false, true, false))
        assertFalse(NativeTopicSendGuard.allowsSend(true, false, false))
        assertFalse(NativeTopicSendGuard.allowsSend("true", true, false))
        assertFalse(NativeTopicSendGuard.allowsSend(true, "true", false))
        assertFalse(NativeTopicSendGuard.allowsSend(1, true, false))
        assertFalse(NativeTopicSendGuard.allowsSend(true, 1, false))
        assertFalse(NativeTopicSendGuard.allowsSend(null, true, false))
        assertFalse(NativeTopicSendGuard.allowsSend(true, null, false))
    }
}
