package org.vcptoolbox.nativeclient

/** Fail closed. JSONObject.optBoolean() coerces strings such as "true", so it
 * must never be the source of submission permission. Use only JSON booleans. */
internal object NativeTopicSendGuard {
    fun allowsSend(historyReady: Any?, executable: Any?, hasMore: Boolean): Boolean =
        !hasMore && historyReady is Boolean && executable is Boolean && historyReady && executable
}
