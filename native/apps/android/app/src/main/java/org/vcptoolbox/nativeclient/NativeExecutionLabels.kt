package org.vcptoolbox.nativeclient

/** Display-only state strings. A history execution is NOT an original-send ACK,
 * an AA completion certificate, or permission to cancel another session's task.
 * Never include execution IDs, content, request payload or extensions here. */
internal object NativeExecutionLabels {
    const val MAX_JSON_UTF8_BYTES = 131_072
    const val MAX_VISIBLE = 24
    private val safeState = Regex("[A-Za-z0-9_-]{1,48}")

    /** `states` must already be the selected `state` field of server objects.
     * `total` is the size of the array, not a count of successful executions. */
    fun render(states: List<String?>, total: Int): List<String> {
        require(total >= 0 && states.size <= MAX_VISIBLE && states.size <= total)
        val preview = states.map { raw ->
            if (raw != null && safeState.matches(raw)) "服务端状态：$raw" else "服务端状态：未识别"
        }.toMutableList()
        if (total > states.size) preview += "另有 ${total - states.size} 项状态未显示"
        return preview
    }
}
